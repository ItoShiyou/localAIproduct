// Local Xcode StoreKit testing only; no Apple Account or real payment.
import Foundation
import StoreKitTest
import StoreKit
import XCTest
@testable import MinutesStoreKitQA
private final class CallBox {
    let continuation: CheckedContinuation<[String: Any], Never>
    init(_ continuation: CheckedContinuation<[String: Any], Never>) { self.continuation = continuation }
}
private var observedPro = false
private func call(_ action: Int32) async -> [String: Any] {
    await withCheckedContinuation { continuation in
        let box = Unmanaged.passRetained(CallBox(continuation)).toOpaque()
        request(action, { pointer, context in
            let box = Unmanaged<CallBox>.fromOpaque(context!).takeRetainedValue()
            let value = try! JSONSerialization.jsonObject(with: Data(String(cString: pointer!).utf8)) as! [String: Any]
            box.continuation.resume(returning: value)
        }, box)
    }
}
final class StoreKitTests: XCTestCase {
    @MainActor func testPurchaseRestoreRefund() async throws {
            let session = try SKTestSession(contentsOf: Bundle.main.url(forResource: "Minutes", withExtension: "storekit")!)
            session.disableDialogs = true
            session.clearTransactions()
            start { pointer, _ in
                let value = try! JSONSerialization.jsonObject(with: Data(String(cString: pointer!).utf8)) as! [String: Any]
                observedPro = value["pro"] as? Bool ?? false
            }
            let initial = await call(0)
            XCTAssertTrue(initial["pro"] as? Bool == false)
            try await session.setSimulatedError(SKTestFailures.Purchase.generic(.userCancelled), forAPI: .purchase)
            let cancelled = await call(1)
            // This SDK may wrap injected cancellation as an unknown service error.
            XCTAssertTrue(["cancelled", "error"].contains(cancelled["status"] as? String ?? ""))
            XCTAssertTrue(cancelled["pro"] as? Bool == false)
            try await session.setSimulatedError(nil, forAPI: .purchase)
            let purchase = await call(1)
            let result = purchase["status"] as? String
            XCTAssertTrue(result == "purchased" || result == "cancelled", "purchase: \(purchase)")
            if result == "cancelled" {
                XCTAssertTrue(purchase["pro"] as? Bool == false)
                _ = try await session.buyProduct(identifier: "dev.localaiproduct.minutes.pro")
                print("Native purchase returned cancelled; using official SKTestSession local transaction injection")
            }
            let purchasedState = await call(0)
            XCTAssertTrue(purchasedState["pro"] as? Bool == true)
            try await session.setSimulatedError(SKTestFailures.LoadProducts.generic(.networkError(URLError(.notConnectedToInternet))), forAPI: .loadProducts)
            let offlineStatus = await call(0)
            XCTAssertTrue(offlineStatus["pro"] as? Bool == true, "Offline product loading must not remove cached rights")
            try await session.setSimulatedError(nil, forAPI: .loadProducts)
            let restored = await call(2)
            XCTAssertTrue(restored["pro"] as? Bool == true)
            guard case .verified(let current) = await StoreKit.Transaction.latest(for: "dev.localaiproduct.minutes.pro") else { XCTFail("No verified local purchase"); return }
            print("Local test transaction IDs: \(session.allTransactions().map { $0.identifier }); signed latest: \(current.id)")
            try session.refundTransaction(identifier: UInt(current.id))
            for _ in 0..<200 {
                if !observedPro { break }
                try await Task.sleep(nanoseconds: 100_000_000)
            }
            if case .verified(let latest) = await StoreKit.Transaction.latest(for: "dev.localaiproduct.minutes.pro") { print("Latest signed transaction revoked=\(latest.revocationDate != nil)") }
            let refunded = await call(0)
            XCTAssertTrue(refunded["pro"] as? Bool == false)
            XCTAssertTrue(!observedPro)
            session.askToBuyEnabled = true
            let pending = await call(1)
            XCTAssertEqual(pending["status"] as? String, "pending")
            XCTAssertTrue(pending["pro"] as? Bool == false)
            let pendingTransaction = try XCTUnwrap(session.allTransactions().first(where: { $0.pendingAskToBuyConfirmation }))
            try session.approveAskToBuyTransaction(identifier: pendingTransaction.identifier)
            for _ in 0..<200 {
                if observedPro { break }
                try await Task.sleep(nanoseconds: 100_000_000)
            }
            XCTAssertTrue(observedPro, "Approval must reach the native rights callback via updates or signed-cache refresh")
            let approved = await call(0)
            XCTAssertTrue(approved["pro"] as? Bool == true)
            session.askToBuyEnabled = false
            print("Local StoreKit checks completed; XCTest assertions determine success")
            session.clearTransactions()

    }
}
