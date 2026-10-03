use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    /// モデルの出力がJSONとして読めない、または形が違う
    #[error("読み取り結果を解釈できません: {0}")]
    Parse(String),
    /// 通信できない(オフライン、接続拒否、タイムアウト)
    #[error("通信できません: {0}")]
    Network(String),
    /// サーバーが明示的に拒否した(無効なキー、無効化されたキーなど)
    #[error("拒否されました: {0}")]
    Rejected(String),
    /// 文字コードの変換ができない
    #[error("文字コードを変換できません: {0}")]
    Encoding(String),
    /// CSVの組み立てに失敗
    #[error("CSVを作成できません: {0}")]
    Csv(String),
}
