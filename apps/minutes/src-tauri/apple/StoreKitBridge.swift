import Foundation
import StoreKit

// No app-managed entitlement file: StoreKit owns the signed offline transaction cache.
private let productID = Bundle.main.object(forInfoDictionaryKey: "MinutesProProductID") as? String ?? "dev.localaiproduct.minutes.pro"
private typealias Reply = @convention(c) (UnsafePointer<CChar>?, UnsafeMutableRawPointer?) -> Void
private var observer: Task<Void, Never>?
private var refreshTask: Task<Void, Never>?
private var entitlementReply: Reply?
// A verified revocation wins over an older entry still present in StoreKit's cache.
private actor VerifiedRights {
    static let shared = VerifiedRights()
    private var activeIDs: Set<UInt64> = []
    private var revokedIDs: Set<UInt64> = []
    func accept(_ transaction: Transaction) {
        guard transaction.productID == productID, transaction.productType == .nonConsumable else { return }
        if transaction.revocationDate != nil {
            revokedIDs.insert(transaction.id)
            activeIDs.remove(transaction.id)
        } else if !revokedIDs.contains(transaction.id) {
            activeIDs.insert(transaction.id)
        }
    }
    func isPro() -> Bool { !activeIDs.isEmpty }
}

private func entitled() async -> Bool {
    // Signed latest transactions include refunds; currentEntitlements supplies the offline cache.
    if case .verified(let latest) = await Transaction.latest(for: productID) {
        await VerifiedRights.shared.accept(latest)
    }
    for await result in Transaction.currentEntitlements {
        if case .verified(let transaction) = result {
            await VerifiedRights.shared.accept(transaction)
        }
    }
    return await VerifiedRights.shared.isPro()
}
private func send(_ reply: Reply, _ context: UnsafeMutableRawPointer?, _ status: String, _ pro: Bool, _ price: String? = nil) {
    var value: [String: Any] = ["status": status, "pro": pro]
    if let price = price { value["price"] = price }
    let data = try! JSONSerialization.data(withJSONObject: value)
    String(decoding: data, as: UTF8.self).withCString { reply($0, context) }
}

@_cdecl("minutes_storekit_start")
public func start(_ reply: @escaping @convention(c) (UnsafePointer<CChar>?, UnsafeMutableRawPointer?) -> Void) {
    entitlementReply = reply
    observer?.cancel()
    refreshTask?.cancel()
    // StoreKit updates are asynchronous. Refresh only signed rights (no price request)
    // so approval/revocation is also reflected if a notification arrives late.
    refreshTask = Task {
        while !Task.isCancelled {
            do { try await Task.sleep(nanoseconds: 5_000_000_000) } catch { return }
            send(reply, nil, "refreshed", await entitled())
        }
    }
    observer = Task {
        send(reply, nil, "ready", await entitled())
        for await result in Transaction.updates {
            if case .verified(let transaction) = result {
                if transaction.productID == productID {
                    await VerifiedRights.shared.accept(transaction)
                    send(reply, nil, "updated", await entitled())
                    await transaction.finish()
                }
            }
        }
    }
}

@_cdecl("minutes_storekit_request")
public func request(_ action: Int32, _ reply: @escaping @convention(c) (UnsafePointer<CChar>?, UnsafeMutableRawPointer?) -> Void, _ context: UnsafeMutableRawPointer?) {
    Task { @MainActor in
        do {
            if action == 2 { try await AppStore.sync() }
            if action == 1 {
                guard let product = try await Product.products(for: [productID]).first,
                      product.type == .nonConsumable else {
                    send(reply, context, "unavailable", await entitled()); return
                }
                switch try await product.purchase() {
                case .success(let result):
                    guard case .verified(let transaction) = result,
                          transaction.productID == productID,
                          transaction.revocationDate == nil else {
                        send(reply, context, "unverified", await entitled()); return
                    }
                    await VerifiedRights.shared.accept(transaction)
                    let pro = await entitled()
                    if let update = entitlementReply { send(update, nil, "updated", pro) }
                    await transaction.finish()
                    send(reply, context, "purchased", pro, product.displayPrice)
                case .userCancelled: send(reply, context, "cancelled", await entitled())
                case .pending: send(reply, context, "pending", await entitled())
                @unknown default: send(reply, context, "unavailable", await entitled())
                }
            } else {
                let pro = await entitled()
                if let update = entitlementReply { send(update, nil, "updated", pro) }
                let price = try? await Product.products(for: [productID]).first?.displayPrice
                send(reply, context, action == 2 ? "restored" : "ready", pro, price ?? nil)
            }
        } catch {
            // Do not emit account identifiers, receipt contents, or raw StoreKit errors.
            if let storeError = error as? StoreKitError, case .userCancelled = storeError {
                send(reply, context, "cancelled", await entitled())
            } else {
                send(reply, context, "error", await entitled())
            }
        }
    }
}
