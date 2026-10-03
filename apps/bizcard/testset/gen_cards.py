import json, os, unicodedata
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "cards")
os.makedirs(OUT, exist_ok=True)
for f in os.listdir(OUT): os.remove(os.path.join(OUT, f))

def cw(ch):
    return 1.0 if unicodedata.east_asian_width(ch) in "WF" else 0.55
def H(text, x, y, s, conf=0.97):
    w = round(sum(cw(c) for c in text) * s)
    return {"text": text, "bbox": {"x": x, "y": y, "w": w, "h": s}, "vertical": False, "confidence": conf}
def V(text, x, y, s, conf=0.95):
    h = round(sum(1.0 for c in text) * s)
    return {"text": text, "bbox": {"x": x, "y": y, "w": s, "h": h}, "vertical": True, "confidence": conf}

def exp(**kw):
    base = dict(name=None, name_kana=None, company=None, department=None, title=None,
                emails=[], phones=[], mobiles=[], faxes=[], postal_code=None, address=None, urls=[])
    base.update(kw); return base

cards = []
def card(cid, side, tags, lines, expected, hard=False, note=""):
    cards.append((cid, side, tags, lines, expected, hard, note))

# 01 横書き標準
card("01", "front", ["horizontal","ja"], [
 H("株式会社ひなた工房", 60, 50, 30),
 H("制作部 デザイン課", 60, 100, 20),
 H("課長", 60, 130, 20),
 H("青木 遥", 60, 190, 52),
 H("あおき はるか", 60, 250, 18),
 H("〒000-0001 東京都千代田区1-2-3 サンプルビル5F", 60, 400, 18),
 H("TEL 03-0000-1101 FAX 03-0000-1102", 60, 430, 18),
 H("携帯 090-0000-1101", 60, 460, 18),
 H("E-mail aoki@hinata-kobo.example.com", 60, 490, 18),
 H("https://hinata-kobo.example.com", 60, 520, 18),
], exp(name="青木 遥", name_kana="あおき はるか", company="株式会社ひなた工房", department="制作部 デザイン課", title="課長",
       emails=["aoki@hinata-kobo.example.com"], phones=["03-0000-1101"], mobiles=["090-0000-1101"], faxes=["03-0000-1102"],
       postal_code="000-0001", address="東京都千代田区1-2-3 サンプルビル5F", urls=["https://hinata-kobo.example.com"]))

# 02 部署+役職が同一行
card("02", "front", ["horizontal","ja"], [
 H("合同会社みなと設計", 80, 60, 28),
 H("営業部 部長", 80, 120, 20),
 H("井上 健太", 80, 180, 48),
 H("イノウエ ケンタ", 80, 236, 16),
 H("〒000-0002 神奈川県横浜市中区4-5-6", 80, 380, 18),
 H("Tel: 045-000-2201", 80, 410, 18),
 H("Mail: inoue@minato-sekkei.example.org", 80, 440, 18),
 H("www.minato-sekkei.example.org", 80, 470, 18),
], exp(name="井上 健太", name_kana="イノウエ ケンタ", company="合同会社みなと設計", department="営業部", title="部長",
       emails=["inoue@minato-sekkei.example.org"], phones=["045-000-2201"], postal_code="000-0002",
       address="神奈川県横浜市中区4-5-6", urls=["www.minato-sekkei.example.org"]))

# 03 営業部長 結合 + 代表取締役
card("03", "front", ["horizontal","ja"], [
 H("有限会社あさひ商会", 60, 50, 30),
 H("代表取締役", 60, 110, 22),
 H("森田 一郎", 60, 160, 50),
 H("専務取締役", 560, 110, 20),
 H("営業部長 石川 美咲", 560, 150, 24),
 H("〒000-0003 大阪府大阪市北区7-8-9 テスト会館2階", 60, 400, 18),
 H("電話 06-0000-3301", 60, 430, 18),
 H("morita@asahi-shokai.example.com", 60, 460, 18),
], exp(name="森田 一郎", company="有限会社あさひ商会", title="代表取締役",
       emails=["morita@asahi-shokai.example.com"], phones=["06-0000-3301"], postal_code="000-0003",
       address="大阪府大阪市北区7-8-9 テスト会館2階"), hard=True, note="同一面に2人分の表記があるが正解は左の1人(大きい文字)のみ。右側の2人目は対象外")

# 04 全角数字と(株)
card("04", "front", ["horizontal","ja","fullwidth"], [
 H("(株)つばき運輸", 70, 50, 28),
 H("物流企画室 係長", 70, 100, 20),
 H("小林 大地", 70, 160, 46),
 H("〒０００−０００４ 愛知県名古屋市中区１−１０−１", 70, 380, 18),
 H("ＴＥＬ：０５２−０００−４４０１　ＦＡＸ：０５２−０００−４４０２", 70, 410, 18),
 H("Ｅ−ｍａｉｌ：kobayashi＠tsubaki-unyu.example.net", 70, 440, 18),
], exp(name="小林 大地", company="(株)つばき運輸", department="物流企画室", title="係長",
       emails=["kobayashi@tsubaki-unyu.example.net"], phones=["052-000-4401"], faxes=["052-000-4402"],
       postal_code="000-0004", address="愛知県名古屋市中区1-10-1"))

# 05 縦書き
card("05", "front", ["vertical","ja"], [
 V("株式会社さくら食品", 920, 60, 28),
 V("取締役", 860, 60, 20),
 V("渡辺 優子", 790, 140, 52),
 V("わたなべ ゆうこ", 740, 140, 16),
 V("〒000-0005 福岡県福岡市博多区2-3-4", 120, 60, 18),
 V("電話 092-000-5501", 80, 60, 18),
 V("watanabe@sakura-shokuhin.example.com", 40, 60, 16),
], exp(name="渡辺 優子", name_kana="わたなべ ゆうこ", company="株式会社さくら食品", title="取締役",
       emails=["watanabe@sakura-shokuhin.example.com"], phones=["092-000-5501"], postal_code="000-0005",
       address="福岡県福岡市博多区2-3-4"))

# 06 縦書き 部署あり
card("06", "front", ["vertical","ja"], [
 V("一般社団法人あおぞら文化協会", 940, 60, 24),
 V("企画部", 890, 60, 18),
 V("事務局長", 860, 60, 20),
 V("高橋 誠", 780, 120, 56),
 V("〒000-0006 北海道札幌市中央区5-6-7", 150, 60, 18),
 V("TEL 011-000-6601", 110, 60, 18),
 V("FAX 011-000-6602", 75, 60, 18),
 V("takahashi@aozora-bunka.example.org", 40, 60, 16),
], exp(name="高橋 誠", company="一般社団法人あおぞら文化協会", department="企画部", title="事務局長",
       emails=["takahashi@aozora-bunka.example.org"], phones=["011-000-6601"], faxes=["011-000-6602"],
       postal_code="000-0006", address="北海道札幌市中央区5-6-7"))

# 07 英語
card("07", "front", ["horizontal","en"], [
 H("Larkspur Analytics Inc.", 60, 50, 30),
 H("Jordan Ellis", 60, 150, 52),
 H("Director of Operations", 60, 215, 22),
 H("1200 Example Avenue, Suite 400", 60, 380, 18),
 H("Springfield, ST 00000", 60, 408, 18),
 H("T: (555) 010-0147", 60, 440, 18),
 H("jordan.ellis@larkspur-analytics.example.com", 60, 470, 18),
 H("www.larkspur-analytics.example.com", 60, 500, 18),
], exp(name="Jordan Ellis", company="Larkspur Analytics Inc.", title="Director of Operations",
       emails=["jordan.ellis@larkspur-analytics.example.com"], phones=["(555) 010-0147"],
       address="1200 Example Avenue, Suite 400 Springfield, ST 00000", urls=["www.larkspur-analytics.example.com"]),
     hard=True, note="住所が2行に分かれる(連結の期待は『住所行+次行』)")

# 08 英語 Mobile/Fax 、 ロングタイトル
card("08", "front", ["horizontal","en"], [
 H("Brightwater Software Co., Ltd.", 60, 50, 28),
 H("Morgan Reyes", 60, 140, 50),
 H("Senior Software Engineer", 60, 200, 22),
 H("Platform Engineering Department", 60, 230, 18),
 H("Tel +1-555-010-0182   Fax +1-555-010-0183", 60, 400, 18),
 H("Mobile +1-555-010-0184", 60, 430, 18),
 H("morgan.reyes@brightwater.example.com", 60, 460, 18),
 H("https://brightwater.example.com/team", 60, 490, 18),
], exp(name="Morgan Reyes", company="Brightwater Software Co., Ltd.", department="Platform Engineering Department",
       title="Senior Software Engineer", emails=["morgan.reyes@brightwater.example.com"],
       phones=["+1-555-010-0182"], mobiles=["+1-555-010-0184"], faxes=["+1-555-010-0183"],
       urls=["https://brightwater.example.com/team"]))

# 09 01の人物の裏面(英語表記)
card("09", "back", ["horizontal","en","back"], [
 H("Hinata Kobo Co., Ltd.", 60, 50, 28),
 H("Design Section", 60, 100, 18),
 H("Haruka Aoki", 60, 160, 46),
 H("Manager", 60, 215, 20),
 H("1-2-3 Sample Building 5F, Chiyoda-ku, Tokyo 000-0001, Japan", 60, 400, 16),
 H("Tel: +81-3-0000-1101  Fax: +81-3-0000-1102", 60, 430, 16),
 H("Mobile: +81-90-0000-1101", 60, 455, 16),
 H("aoki@hinata-kobo.example.com", 60, 480, 16),
], exp(name="Haruka Aoki", company="Hinata Kobo Co., Ltd.", department="Design Section", title="Manager",
       emails=["aoki@hinata-kobo.example.com"], phones=["+81-3-0000-1101"], mobiles=["+81-90-0000-1101"],
       faxes=["+81-3-0000-1102"], address="1-2-3 Sample Building 5F, Chiyoda-ku, Tokyo 000-0001, Japan"),
     hard=True, note="海外式の住所表記。郵便番号は住所内に埋まる(postal_code は期待しない)")

# 10 裏面: 氏名なし
card("10", "back", ["horizontal","ja","back"], [
 H("株式会社ひなた工房", 80, 60, 30),
 H("私たちの仕事", 80, 130, 22),
 H("・ウェブサイト制作", 80, 170, 18),
 H("・パンフレット・ロゴのデザイン", 80, 200, 18),
 H("・撮影ディレクション", 80, 230, 18),
 H("https://hinata-kobo.example.com", 80, 500, 18),
], exp(company="株式会社ひなた工房", urls=["https://hinata-kobo.example.com"]),
     note="氏名のない裏面。氏名を誤って拾わないこと")

# 11 漢字名+ローマ字
card("11", "front", ["horizontal","ja","en"], [
 H("株式会社かもめ物産", 60, 50, 30),
 H("海外事業部 マネージャー", 60, 100, 20),
 H("佐々木 翔太", 60, 160, 50),
 H("Shota Sasaki", 60, 220, 20),
 H("〒000-0007 兵庫県神戸市中央区3-3-3", 60, 400, 18),
 H("TEL 078-000-7701", 60, 430, 18),
 H("sasaki@kamome-bussan.example.com", 60, 460, 18),
], exp(name="佐々木 翔太", company="株式会社かもめ物産", department="海外事業部", title="マネージャー",
       emails=["sasaki@kamome-bussan.example.com"], phones=["078-000-7701"], postal_code="000-0007",
       address="兵庫県神戸市中央区3-3-3"))

# 12 OCR誤認識(カンマ)
card("12", "front", ["horizontal","ja","ocr-noise"], [
 H("株式会社やまびこ電機", 60, 50, 30),
 H("技術部 主任", 60, 100, 20),
 H("松本 里奈", 60, 160, 50),
 H("TEL 0263-00-8801", 60, 400, 18),
 H("matsumoto@yamabiko-denki,example.com", 60, 430, 18),
 H("〒000-0008 長野県松本市1-1-1", 60, 460, 18),
], exp(name="松本 里奈", company="株式会社やまびこ電機", department="技術部", title="主任",
       emails=["matsumoto@yamabiko-denki.example.com"], phones=["0263-00-8801"], postal_code="000-0008",
       address="長野県松本市1-1-1"),
     hard=True, note="OCRが '.' を ',' に誤認識した想定。ルールでは直せないので期待は不一致になりうる")

# 13 キャッチコピーが一番大きい
card("13", "front", ["horizontal","ja","decorative"], [
 H("暮らしに、ひと工夫を。", 60, 40, 44),
 H("株式会社こもれび家具", 60, 110, 24),
 H("営業部", 60, 150, 18),
 H("中村 恵", 60, 210, 36),
 H("TEL 075-000-9901", 60, 400, 18),
 H("nakamura@komorebi-kagu.example.com", 60, 430, 18),
 H("〒000-0009 京都府京都市中京区9-9-9", 60, 460, 18),
], exp(name="中村 恵", company="株式会社こもれび家具", department="営業部",
       emails=["nakamura@komorebi-kagu.example.com"], phones=["075-000-9901"], postal_code="000-0009",
       address="京都府京都市中京区9-9-9"),
     hard=True, note="キャッチコピーが氏名より大きい装飾デザインの想定")

# 14 会社なし(個人事業)
card("14", "front", ["horizontal","ja"], [
 H("デザイナー", 80, 120, 20),
 H("藤田 大輔", 80, 160, 52),
 H("フジタ ダイスケ", 80, 220, 16),
 H("090-0000-1401", 80, 400, 18),
 H("fujita@fujita-design.example.net", 80, 430, 18),
 H("https://fujita-design.example.net", 80, 460, 18),
], exp(name="藤田 大輔", name_kana="フジタ ダイスケ", title="デザイナー", mobiles=["090-0000-1401"],
       emails=["fujita@fujita-design.example.net"], urls=["https://fujita-design.example.net"]))

# 15 メール複数、英語大文字
card("15", "front", ["horizontal","ja","en"], [
 H("株式会社グリーンリーフ", 60, 50, 30),
 H("代表取締役社長", 60, 100, 20),
 H("伊藤 沙織", 60, 150, 50),
 H("SAORI ITO", 60, 210, 18),
 H("main: ito@greenleaf.example.com", 60, 400, 18),
 H("sub: saori.ito@greenleaf.example.com", 60, 430, 18),
 H("TEL 03-0000-1501 / 080-0000-1502", 60, 460, 18),
], exp(name="伊藤 沙織", company="株式会社グリーンリーフ", title="代表取締役社長",
       emails=["ito@greenleaf.example.com","saori.ito@greenleaf.example.com"],
       phones=["03-0000-1501"], mobiles=["080-0000-1502"]))

# 16 カナ名・ひらがな会社なし表記の英語名刺 (Dept/Title 別行)
card("16", "front", ["horizontal","en"], [
 H("Cedar & Pine LLC", 70, 50, 30),
 H("Alex Kim", 70, 140, 50),
 H("Founder", 70, 200, 22),
 H("alex@cedarpine.example.org", 70, 380, 18),
 H("+1 555 010 0166", 70, 410, 18),
], exp(name="Alex Kim", company="Cedar & Pine LLC", title="Founder",
       emails=["alex@cedarpine.example.org"], phones=["+1 555 010 0166"]))

# 17 縦書き 低コントラスト 信頼度低
card("17", "front", ["vertical","ja","low-confidence"], [
 V("株式会社うみねこ水産", 900, 70, 26, 0.71),
 V("営業部長", 850, 70, 20, 0.68),
 V("加藤 直樹", 770, 140, 54, 0.74),
 V("〒000-0017 宮城県仙台市青葉区1-7-7", 140, 70, 18, 0.62),
 V("TEL 022-000-1701", 100, 70, 18, 0.66),
 V("kato@umineko-suisan.example.com", 60, 70, 16, 0.6),
], exp(name="加藤 直樹", company="株式会社うみねこ水産", department="営業部", title="部長",
       emails=["kato@umineko-suisan.example.com"], phones=["022-000-1701"], postal_code="000-0017",
       address="宮城県仙台市青葉区1-7-7"), hard=True, note="OCR信頼度が低い想定(0.6〜0.74)。『営業部長』は 部署=営業部・役職=部長 に分ける期待")

# 18 TEL 括弧付き市外局番、URL ラベル
card("18", "front", ["horizontal","ja"], [
 H("株式会社そよ風ベーカリー", 60, 50, 28),
 H("店長", 60, 100, 20),
 H("山口 朋美", 60, 150, 48),
 H("〒000-0018 広島県広島市中区2-2-2", 60, 400, 18),
 H("TEL (082) 000-1801", 60, 430, 18),
 H("URL: soyokaze-bakery.example.com", 60, 460, 18),
 H("info@soyokaze-bakery.example.com", 60, 490, 18),
], exp(name="山口 朋美", company="株式会社そよ風ベーカリー", title="店長",
       emails=["info@soyokaze-bakery.example.com"], phones=["(082) 000-1801"], postal_code="000-0018",
       address="広島県広島市中区2-2-2", urls=["soyokaze-bakery.example.com"]))


# --- 以下は、ルールが想定していない形を意図的に混ぜた「難しい」名刺(正解は人が付ける想定の値) ---
# 19 会社名に法人格の語がない
card("19", "front", ["horizontal","ja"], [
 H("サンプル商事", 60, 50, 32),
 H("営業部 課長", 60, 110, 20),
 H("長谷川 亮", 60, 170, 50),
 H("TEL 03-0000-1901", 60, 400, 18),
 H("hasegawa@sample-shoji.example.com", 60, 430, 18),
], exp(name="長谷川 亮", company="サンプル商事", department="営業部", title="課長",
       emails=["hasegawa@sample-shoji.example.com"], phones=["03-0000-1901"]),
     hard=True, note="会社名に法人格の語がない(ルールは拾えない想定)")

# 20 姓が「阿部」(部で終わる)
card("20", "front", ["horizontal","ja"], [
 H("株式会社ことり印刷", 60, 50, 28),
 H("阿部 花子", 60, 130, 50),
 H("アベ ハナコ", 60, 190, 16),
 H("営業部 担当", 60, 230, 20),
 H("abe@kotori-insatsu.example.com", 60, 400, 18),
], exp(name="阿部 花子", name_kana="アベ ハナコ", company="株式会社ことり印刷", department="営業部", title="担当",
       emails=["abe@kotori-insatsu.example.com"]),
     note="姓が部で終わっても部署と誤認しないこと")

# 21 カタカナ名(中黒)
card("21", "front", ["horizontal","ja"], [
 H("株式会社ブルーレイク", 60, 50, 28),
 H("ジョン・スミス", 60, 130, 48),
 H("海外営業部 部長", 60, 200, 20),
 H("john.smith@bluelake.example.com", 60, 400, 18),
], exp(name="ジョン・スミス", company="株式会社ブルーレイク", department="海外営業部", title="部長",
       emails=["john.smith@bluelake.example.com"]),
     hard=True, note="カタカナの外国人名(中黒入り)。ルールは漢字名と英字名だけを氏名の形として扱う")

# 22 ハイフンなしの電話番号
card("22", "front", ["horizontal","ja"], [
 H("株式会社くじら企画", 60, 50, 28),
 H("坂本 陽介", 60, 130, 50),
 H("TEL 0300001222 / 09000001223", 60, 400, 18),
 H("sakamoto@kujira-kikaku.example.com", 60, 430, 18),
], exp(name="坂本 陽介", company="株式会社くじら企画", phones=["0300001222"], mobiles=["09000001223"],
       emails=["sakamoto@kujira-kikaku.example.com"]),
     hard=True, note="ハイフンのない電話番号。ルールはハイフン区切りだけを拾う")

# 23 メールの途中に空白(OCRの分割)
card("23", "front", ["horizontal","en","ocr-noise"], [
 H("Quartz Labs Ltd.", 60, 50, 28),
 H("Riley Chen", 60, 130, 50),
 H("CTO", 60, 190, 22),
 H("riley.chen @ quartzlabs.example.com", 60, 400, 18),
], exp(name="Riley Chen", company="Quartz Labs Ltd.", title="CTO", emails=["riley.chen@quartzlabs.example.com"]),
     hard=True, note="OCRがメールの@の前後に空白を入れた想定")

# 24 会社名が2行に分かれる
card("24", "front", ["horizontal","ja"], [
 H("株式会社ひまわり", 60, 50, 28),
 H("システム開発", 60, 84, 28),
 H("森 康介", 60, 170, 50),
 H("TEL 03-0000-2401", 60, 400, 18),
], exp(name="森 康介", company="株式会社ひまわりシステム開発", phones=["03-0000-2401"]),
     hard=True, note="会社名が2行に折り返された想定。ルールは1行ずつ見る")

# 25 縦書きの電話に全角ハイフン相当(長音)
card("25", "front", ["vertical","ja","ocr-noise"], [
 V("株式会社なぎさ観光", 920, 60, 26),
 V("代表取締役", 870, 60, 20),
 V("岡田 蓮", 790, 120, 54),
 V("電話 0 3 - 0 0 0 0 - 2 5 0 1", 100, 60, 18),
], exp(name="岡田 蓮", company="株式会社なぎさ観光", title="代表取締役", phones=["03-0000-2501"]),
     hard=True, note="縦書きでOCRが文字ごとに空白を入れた想定")

for cid, side, tags, lines, e, hard, note in cards:
    meta = {"id": cid, "side": side, "width": 1050, "height": 600, "tags": tags, "hard": hard, "note": note,
            "fictional": True,
            "source": "手で書いた架空のデータ。実画像・実OCR出力ではない(OCRの出力形式を模したもの)",
            "lines": lines}
    json.dump(meta, open(f"{OUT}/{cid}.ocr.json", "w"), ensure_ascii=False, indent=1)
    json.dump(e, open(f"{OUT}/{cid}.expected.json", "w"), ensure_ascii=False, indent=1)
print(len(cards))
