# 依存ライブラリとモデルのライセンス

確認日: 2026-10-03。ライセンス表記は、クレート・パッケージのメタデータと配布物の同梱ファイルで確認した範囲。
法的な判断ではない。配布前に、各ライセンス全文と通知(NOTICE)を同梱物に入れる作業が要る。
**確認できないものは使わず、「要確認」に書いてある。**

## 製品に入れるもの

### Rust クレート(`core/engine` と `src-tauri`、`cargo metadata` で全依存を確認)

直接の依存だけを挙げる。推移的な依存(約140件)も、すべて下のいずれかの許諾で、**GPL・AGPL・LGPL は含まれない**ことを確認した。

| 名前 | バージョン | ライセンス | 配布時に必要な表記 |
|---|---|---|---|
| serde / serde_json | 1.x | MIT OR Apache-2.0 | 著作権表示とライセンス全文 |
| thiserror | 1.x | MIT OR Apache-2.0 | 同上 |
| csv | 1.x | Unlicense OR MIT | 同上 |
| encoding_rs | 0.8 | (Apache-2.0 OR MIT) AND BSD-3-Clause | 同上(BSD-3 の著作権表示) |
| base64 / regex | 0.22 / 1.x | MIT OR Apache-2.0 | 同上 |
| ureq(ライセンス認証・更新確認の通信用) | 2.x | MIT OR Apache-2.0 | 同上。依存の rustls は Apache-2.0 OR ISC OR MIT、ring は Apache-2.0 AND ISC |
| image(PNG/JPEG の読み込み) | 0.25 | MIT OR Apache-2.0 | 同上。JPEG の zune-jpeg は MIT OR Apache-2.0 OR Zlib |
| ort(ONNX Runtime の Rust 用ラッパー、`onnx` 機能) | =2.0.0-rc.13 | MIT OR Apache-2.0 | 同上。**リリース候補版**で、API が変わりうるので版を固定している |
| ndarray | 0.17 | MIT OR Apache-2.0 | 同上 |
| rusqlite(`bundled` で SQLite を同梱) | 0.40 | MIT | 同上 |
| SQLite 本体(rusqlite が同梱、3.53.2) | 3.53 | パブリックドメイン | 不要(任意で記載) |
| unicode-normalization | 0.1 | MIT OR Apache-2.0 | 同上。依存の ICU データは Unicode-3.0(著作権表示と許諾文の同梱が必要) |
| pdf-extract(PDFのテキスト層。`src-tauri` で使用) | 0.12.1 | MIT | 同上 |
| lopdf(画像のみPDFの埋め込みJPEG取り出し) | 0.45 | MIT | 同上 |
| sha2(SHA-256) | 0.10 | MIT OR Apache-2.0 | 同上 |

### 実行時に読み込む共有ライブラリ

| 名前 | バージョン | ライセンス | 配布時に必要な表記 | 備考 |
|---|---|---|---|---|
| ONNX Runtime | 1.30.0(Linux x86_64 の共有ライブラリ 29MB で確認) | MIT | 著作権表示とライセンス全文。ONNX Runtime の THIRD_PARTY_NOTICES の同梱 | macOS(Apple Silicon)・Windows(x64)版の取得と同梱は**未確認**(要確認) |

### モデルと辞書

| 名前 | 用途 | ライセンス | 配布時に必要な表記 | SHA-256(先頭16桁)・サイズ | 取得元 |
|---|---|---|---|---|---|
| ch_PP-OCRv4_det(検出) | 文字領域の検出 | Apache-2.0(PaddleOCR 由来) | Apache-2.0 全文、PaddleOCR への帰属表示 | d2a7720d45a54257 / 4.7MB | `rapidocr-onnxruntime` 1.4.4(PyPI、Apache-2.0)の同梱ファイル。RapidOCR の公開一覧にある `ch_PP-OCRv4_det_mobile.onnx` のハッシュと**一致**を確認 |
| ch_PP-OCRv4_rec(認識、中国語・英数字) | 数字・英字・記号の認識 | Apache-2.0(PaddleOCR 由来) | 同上 | 48fc40f24f6d2a20 / 10.9MB | 同上。`ch_PP-OCRv4_rec_mobile.onnx` のハッシュと**一致**を確認 |
| japan_PP-OCRv3_rec(認識、日本語) と japan_dict.txt | 仮名・漢字の認識 | PaddleOCR 由来で Apache-2.0 の見込み。**再配布パッケージ側は ISC で、モデル自体の表記がない** | 同上 | 329eec1da950c729 / 10.1MB(辞書 0b9c9e34527116e9) | npm `multilingual-purejs-ocr` 1.0.1(ISC)の同梱ファイル。**公式の配布元に通信制限で届かず、ハッシュを公式値と照合できていない → 要確認** |
| jpn.traineddata(Tesseract 用、予備) | 日本語の認識 | Apache-2.0(tesseract-ocr/tessdata_fast・tessdata_best) | Apache-2.0 全文 | fast: 1f5de9236d2e85f5 / 2.4MB、best: 36bdf9ac823f5911 / 14MB | GitHub の raw 配信 / Ubuntu パッケージ |

### 予備の外部実行ファイル(同梱する場合のみ)

| 名前 | バージョン | ライセンス | 備考 |
|---|---|---|---|
| Tesseract | 5.3.4(Ubuntu 版で確認) | Apache-2.0 | `TesseractOcr` が呼ぶ。同梱する場合は Leptonica(BSD-2-Clause 系)など依存ライブラリの表記も要る。Mac・Windows 用ビルドの入手は未確認 |

## 製品に入れないもの(スパイクと開発でだけ使った)

| 名前 | ライセンス | 理由 |
|---|---|---|
| PyMuPDF 1.28.2 | AGPL-3.0 または商用(Artifex) | **製品には使えない**(AGPL)。スパイクで PDF の画像化と比較に使っただけ |
| poppler-utils(`pdftotext`) | GPL | 製品には使わない。確認用のみ |
| RapidOCR(Python)1.4.4 | Apache-2.0 | 製品は Rust 実装。比較のためだけ |
| onnxruntime(Python)1.30.0 | MIT | 共有ライブラリの取り出しと比較のため |
| Pillow(MIT-CMU)、NumPy(BSD-3-Clause ほか)、OpenCV(Apache-2.0)、ReportLab(BSD) | 各右記 | テストセットの生成と比較のみ |

## 要確認

- (方針) japan_PP-OCRv3 は出所確認が済むまで製品に同梱しない。**未解決**

- japan_PP-OCRv3 モデルの出所とライセンス(公式の配布元から取り直してハッシュを照合する。届かない場合は、モデルの出所を PaddleOCR の公式配布で確認できるまで製品に入れない)
- macOS(Apple Silicon)・Windows(x64)用の ONNX Runtime 共有ライブラリの入手経路と同梱方法
- `ort` がリリース候補版(2.0.0-rc.13)であること。安定版が出た時点で更新を検討する
- PDFium(画像のみのPDFの画像化に使う場合)のバイナリの入手経路。BSD-3-Clause / Apache-2.0 の見込みだが、未取得で未確認
