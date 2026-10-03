//! core::ocr のトレイト(FakeOcr)→ ルール抽出 → 保存 → 検索 までを通す。
//! 実OCR(PpOcr)を通した測定は `examples/ocr_eval.rs`(feature onnx、モデルが要る)で行う。

use bizcard_logic::extract::extract;
use bizcard_logic::store::{ImportOutcome, NewEncounter, PersonFields, Store};
use factory_core::ocr::{FakeOcr, Ocr, OcrImage, OcrPage};

fn page(id: &str) -> OcrPage {
    let dir = bizcard_logic::eval::testset_dir();
    serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{id}.ocr.json"))).unwrap()).unwrap()
}

#[test]
fn ocrトレイトから検索まで通る() {
    let ocr = FakeOcr { page: page("01") };
    let img = OcrImage::from_dynamic(image_stub());
    let ext = extract(&ocr.recognize(&img).unwrap());
    let s = Store::open_in_memory().unwrap();
    let ImportOutcome::Imported { person_id, .. } = s.import_card(&"a".repeat(64), "cards/01.png", "front", &ext).unwrap() else { panic!() };
    let (f, _) = PersonFields::from_extraction(&ext);
    s.confirm(person_id, &f, Some(NewEncounter { met_on: Some("2026-10-01".into()), memo: "ロゴの相談".into(), ..Default::default() }), &[]).unwrap();
    let hits = s.search("青木", 5).unwrap(); // 2文字の姓
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].thumbnail_path.as_deref(), Some("cards/01.png"));
    assert_eq!(s.search("ひなた工房", 5).unwrap().len(), 1);
}

fn image_stub() -> image::DynamicImage {
    image::DynamicImage::new_rgb8(8, 8)
}
