/** Must match the Cargo feature selected by tools/build_edition.mjs. */
export const edition = import.meta.env.VITE_MINUTES_EDITION as "free" | "pro" | undefined;
export const purchaseUrl = "https://itoshiyou.github.io/localAIproduct/minutes/purchase/";
export async function openPurchasePage() {
  const tauri = (window as unknown as { __TAURI__?: { core?: { invoke?: (name: string) => Promise<void> } } }).__TAURI__;
  if (tauri?.core?.invoke) await tauri.core.invoke("open_purchase_page");
  else window.open(purchaseUrl, "_blank", "noopener,noreferrer");
}
