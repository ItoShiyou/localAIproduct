/** Must match the Cargo feature selected by tools/build_edition.mjs. */
export const edition = import.meta.env.VITE_MINUTES_EDITION as "free" | "pro" | undefined;
export const purchaseUrl = "https://itoshiyou.github.io/localAIproduct/minutes/purchase/";
export async function openPurchasePage() {
  if (isMacAppStore) throw new Error("購入は設定画面のApple公式決済をご利用ください");
  const tauri = (window as unknown as { __TAURI__?: { core?: { invoke?: (name: string) => Promise<void> } } }).__TAURI__;
  if (tauri?.core?.invoke) await tauri.core.invoke("open_purchase_page");
  else window.open(purchaseUrl, "_blank", "noopener,noreferrer");
}

export const isMacAppStore = import.meta.env.VITE_MINUTES_CHANNEL === "mac-app-store";
export type PurchaseState = { status: string; pro: boolean; price?: string };
export async function storePurchase(action: "status" | "purchase" | "restore"): Promise<PurchaseState> {
  const tauri = (window as unknown as { __TAURI__?: { core?: { invoke?: <T>(name: string, args: unknown) => Promise<T> } } }).__TAURI__;
  if (!tauri?.core?.invoke) throw new Error("Mac App Store版アプリでご利用ください");
  return tauri.core.invoke<PurchaseState>("storekit_request", { action });
}
