//! UI(`ui/src/api.ts` の `Api`)から呼ばれるコマンドの中身。Tauri に依存しない純Rustで、
//! `tauri_glue.rs`(feature `tauri`)の `#[tauri::command]` が、これを1行で呼ぶ。
//! 型は camelCase の JSON(`ui/src/types.ts` と同じ形)。
//!
//! 画像は data URL で受け取り、SHA-256 で同一画像を判定し、アプリのデータフォルダにコピーして保持する
//! (元のファイルは上書きしない)。外部への通信は無い。ログに氏名・メール・電話を書かない。

use crate::dedup::Reason;
use crate::extract::{extract, Extraction};
use crate::store::{ImportOutcome, NewEncounter, PersonFields, Store};
use base64::Engine;
use factory_core::ocr::{Ocr, OcrImage, OcrPage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldsDto {
    pub name: String,
    pub name_kana: String,
    pub company: String,
    pub department: String,
    pub title: String,
    pub email: String,
    pub phone: String,
    pub mobile: String,
    pub postal_code: String,
    pub address: String,
    pub url: String,
}

impl From<PersonFields> for FieldsDto {
    fn from(f: PersonFields) -> Self {
        Self {
            name: f.name, name_kana: f.name_kana, company: f.company, department: f.department, title: f.title,
            email: f.email, phone: f.phone, mobile: f.mobile, postal_code: f.postal_code, address: f.address, url: f.url,
        }
    }
}
impl From<FieldsDto> for PersonFields {
    fn from(f: FieldsDto) -> Self {
        Self {
            name: f.name, name_kana: f.name_kana, company: f.company, department: f.department, title: f.title,
            email: f.email, phone: f.phone, mobile: f.mobile, postal_code: f.postal_code, address: f.address, url: f.url,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateDto {
    pub existing_id: i64,
    pub name: String,
    pub company: String,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CandidateDto {
    pub value: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadResultDto {
    pub person_id: i64,
    pub fields: FieldsDto,
    pub confidence: std::collections::BTreeMap<&'static str, f32>,
    /// 会社名の候補(信頼度の高い順)。確認画面でワンタップ選択できる
    pub company_candidates: Vec<CandidateDto>,
    pub extra_emails: Vec<String>,
    pub duplicates: Vec<DuplicateDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_imported: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterDto {
    pub met_on: String,
    pub place: String,
    pub how_met: String,
    pub memo: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmInput {
    pub person_id: i64,
    pub fields: FieldsDto,
    pub encounter: Option<EncounterDto>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub id: i64,
    pub name: String,
    pub company: String,
    pub title: String,
    /// 画像の data URL(アプリのデータフォルダから読む)
    pub image: Option<String>,
    pub latest_memo: String,
}

pub struct AppState {
    pub store: Mutex<Store>,
    pub ocr: Box<dyn Ocr + Send + Sync>,
    /// 名刺画像のコピーを置くフォルダ(アプリのデータフォルダの中)
    pub image_dir: PathBuf,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn parse_data_url(s: &str) -> Result<(String, Vec<u8>), String> {
    let (head, body) = s.split_once(',').ok_or("画像のデータが不正です")?;
    if !head.starts_with("data:image/") || !head.ends_with(";base64") {
        return Err("対応していない画像形式です(JPEG / PNG)".into());
    }
    let ext = if head.contains("png") { "png" } else { "jpg" };
    let bytes = base64::engine::general_purpose::STANDARD.decode(body.trim()).map_err(err)?;
    Ok((ext.into(), bytes))
}

fn confidences(x: &Extraction) -> std::collections::BTreeMap<&'static str, f32> {
    let mut m = std::collections::BTreeMap::new();
    let mut put = |k: &'static str, f: Option<&crate::extract::Field>| {
        if let Some(f) = f {
            m.insert(k, f.confidence);
        }
    };
    put("name", x.name.as_ref());
    put("nameKana", x.name_kana.as_ref());
    put("company", x.company.as_ref());
    put("department", x.department.as_ref());
    put("title", x.title.as_ref());
    put("email", x.emails.first());
    put("phone", x.phones.first());
    put("mobile", x.mobiles.first());
    put("postalCode", x.postal_code.as_ref());
    put("address", x.address.as_ref());
    put("url", x.urls.first());
    m
}

impl AppState {
    /// 画像を取り込み、OCR→項目抽出→下書きの保存まで行う
    pub fn import_card(&self, image_data_url: &str, side: &str) -> Result<ReadResultDto, String> {
        let (ext, bytes) = parse_data_url(image_data_url)?;
        let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let store = self.store.lock().map_err(err)?;
        let img = OcrImage::decode(&bytes).map_err(err)?;
        let page: OcrPage = self.ocr.recognize(&img).map_err(err)?;
        let x = extract(&page);
        std::fs::create_dir_all(&self.image_dir).map_err(err)?;
        let rel = format!("{sha}.{ext}");
        let path = self.image_dir.join(&rel);
        if !path.exists() {
            std::fs::write(&path, &bytes).map_err(err)?;
        }
        match store.import_card(&sha, &rel, side, &x).map_err(err)? {
            ImportOutcome::AlreadyImported { person_id } => {
                let v = store.person_view(person_id).map_err(err)?;
                Ok(ReadResultDto {
                    person_id,
                    fields: v.fields.into(),
                    confidence: Default::default(),
                    company_candidates: vec![],
                    extra_emails: vec![],
                    duplicates: vec![],
                    already_imported: Some(person_id),
                })
            }
            ImportOutcome::Imported { person_id, duplicates, extra_emails } => {
                let (fields, _) = PersonFields::from_extraction(&x);
                let duplicates = duplicates
                    .into_iter()
                    .map(|d| {
                        let v = store.person_view(d.existing_id).map(|v| v.fields);
                        DuplicateDto {
                            existing_id: d.existing_id,
                            name: v.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
                            company: v.as_ref().map(|f| f.company.clone()).unwrap_or_default(),
                            reasons: d.reasons.iter().map(|r| match r {
                                Reason::SameEmail(_) => "sameEmail",
                                Reason::SameNameAndCompany => "sameNameAndCompany",
                            }).collect(),
                        }
                    })
                    .collect();
                Ok(ReadResultDto {
                    person_id,
                    fields: fields.into(),
                    confidence: confidences(&x),
                    company_candidates: x.company_candidates.iter().map(|f| CandidateDto { value: f.value.clone(), confidence: f.confidence }).collect(),
                    extra_emails, duplicates,
                    already_imported: None,
                })
            }
        }
    }

    pub fn confirm_card(&self, input: ConfirmInput) -> Result<(), String> {
        let enc = input.encounter.map(|e| NewEncounter {
            met_on: if e.met_on.is_empty() { None } else { Some(e.met_on) },
            place: e.place,
            how_met: e.how_met,
            memo: e.memo,
        });
        self.store.lock().map_err(err)?.confirm(input.person_id, &input.fields.into(), enc, &input.tags).map_err(err)
    }

    /// やり直し: 下書きを捨てる。確定済みの人は消さない
    pub fn discard_draft(&self, person_id: i64) -> Result<Vec<String>, String> {
        let store = self.store.lock().map_err(err)?;
        let v = store.person_view(person_id).map_err(err)?;
        if v.status != "draft" {
            return Err("確定済みの人は、ここでは消せません".into());
        }
        let paths = store.delete_person(person_id).map_err(err)?;
        for p in &paths {
            // 他の人の画像と共有していない(SHA-256 は一意)ので、ファイルも消す
            let _ = std::fs::remove_file(self.image_dir.join(p));
        }
        Ok(paths)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchHitDto>, String> {
        let hits = self.store.lock().map_err(err)?.search(query, 50).map_err(err)?;
        Ok(hits
            .into_iter()
            .map(|h| SearchHitDto {
                id: h.id,
                image: h.thumbnail_path.as_ref().and_then(|p| self.image_data_url(p)),
                name: h.name,
                company: h.company,
                title: h.title,
                latest_memo: h.latest_memo,
            })
            .collect())
    }

    fn image_data_url(&self, rel: &str) -> Option<String> {
        let bytes = std::fs::read(self.image_dir.join(rel)).ok()?;
        let mime = if rel.ends_with("png") { "image/png" } else { "image/jpeg" };
        Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factory_core::ocr::FakeOcr;

    fn png_data_url(seed: u8) -> String {
        let mut img = image::RgbImage::new(8, 8);
        img.put_pixel(0, 0, image::Rgb([seed, 0, 0]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
    }

    fn state(dir: &std::path::Path, card: &str) -> AppState {
        let page: OcrPage = serde_json::from_str(&std::fs::read_to_string(crate::eval::testset_dir().join(format!("{card}.ocr.json"))).unwrap()).unwrap();
        AppState { store: Mutex::new(Store::open_in_memory().unwrap()), ocr: Box::new(FakeOcr { page }), image_dir: dir.to_path_buf() }
    }

    #[test]
    fn 取り込み_確定_検索_画像つきで返る() {
        let dir = std::env::temp_dir().join(format!("bizcard-test-{}", std::process::id()));
        let st = state(&dir, "01");
        let url = png_data_url(1);
        let r = st.import_card(&url, "front").unwrap();
        assert_eq!(r.fields.company, "株式会社ひなた工房");
        assert!(r.confidence.contains_key("email"));
        assert_eq!(r.company_candidates[0].value, "株式会社ひなた工房");
        assert!(st.search("青木").unwrap().is_empty(), "確定前は検索に出ない");
        // 同じ画像は取り込まない
        assert_eq!(st.import_card(&url, "front").unwrap().already_imported, Some(r.person_id));
        let mut f = r.fields.clone();
        f.name = "青木 はるか".into();
        st.confirm_card(ConfirmInput {
            person_id: r.person_id,
            fields: f,
            encounter: Some(EncounterDto { met_on: "2026-10-01".into(), place: "展示会".into(), how_met: "".into(), memo: "ロゴの相談".into() }),
            tags: vec![],
        })
        .unwrap();
        let hits = st.search("青木").unwrap(); // 2文字
        assert_eq!(hits.len(), 1);
        assert!(hits[0].image.as_deref().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(hits[0].latest_memo, "ロゴの相談");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn やり直しは下書きだけ消せる() {
        let dir = std::env::temp_dir().join(format!("bizcard-test2-{}", std::process::id()));
        let st = state(&dir, "01");
        let r = st.import_card(&png_data_url(2), "front").unwrap();
        assert_eq!(st.discard_draft(r.person_id).unwrap().len(), 1);
        let r2 = st.import_card(&png_data_url(3), "front").unwrap();
        st.confirm_card(ConfirmInput { person_id: r2.person_id, fields: r2.fields.clone(), encounter: None, tags: vec![] }).unwrap();
        assert!(st.discard_draft(r2.person_id).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 画像でないデータは断る() {
        let dir = std::env::temp_dir().join("bizcard-test3");
        let st = state(&dir, "01");
        assert!(st.import_card("data:text/html;base64,PGI+", "front").is_err());
        assert!(st.import_card("garbage", "front").is_err());
    }

    #[test]
    fn jsonの形はuiのtypes_tsと同じcamelcase() {
        let f = serde_json::to_value(FieldsDto::default()).unwrap();
        for k in ["name", "nameKana", "company", "department", "title", "email", "phone", "mobile", "postalCode", "address", "url"] {
            assert!(f.get(k).is_some(), "{k}");
        }
    }
}
