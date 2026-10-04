import type { ReactNode } from "react";

/** 検索・用語辞書・設定の画面の見出し(小さな英字のラベル + 題 + ひとこと) */
export function PageHead({ label, title, children }: { label: string; title: string; children?: ReactNode }) {
  return (
    <header className="page-head">
      <span className="label">{label}</span>
      <h1>{title}</h1>
      {children && <p>{children}</p>}
    </header>
  );
}
