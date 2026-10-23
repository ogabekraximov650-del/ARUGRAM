//! UZ: Yuqori darajali testlar: JSON -> kadr, rad etish, maska/matte, trim, vaqt.
//! EN: High-level tests: JSON -> frame, rejection, masks/mattes, trim, timing.
//! RU: Тесты верхнего уровня: JSON -> кадр, отклонение, маски/маты, trim, время.

use crate::{Animation, LottieError, RenderConfig, Renderer};

fn render(json: &str, frame: f32) -> crate::RasterFrame {
    let a = Animation::from_json_str(json).expect("parse");
    Renderer::default().render_frame(&a, frame, RenderConfig::default()).expect("render")
}

fn px(f: &crate::RasterFrame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * f.width + x) * 4) as usize;
    [f.pixels[i], f.pixels[i + 1], f.pixels[i + 2], f.pixels[i + 3]]
}

const HEAD: &str = r#""v":"5.7.6","fr":30,"ip":0,"op":30,"w":20,"h":20,"assets":[]"#;
const KS: &str = r#""ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":0,"k":[0,0,0]},"a":{"a":0,"k":[0,0,0]},"s":{"a":0,"k":[100,100,100]}}"#;

fn rect_layer(extra: &str, shapes_extra: &str) -> String {
    format!(
        r#"{{"ddd":0,"ind":1,"ty":4,"nm":"r","sr":1,{KS},"ao":0,"ip":0,"op":30,"st":0,"bm":0{extra},"shapes":[{{"ty":"gr","it":[
        {{"ty":"rc","d":1,"s":{{"a":0,"k":[10,10]}},"p":{{"a":0,"k":[10,10]}},"r":{{"a":0,"k":0}}}},
        {{"ty":"fl","c":{{"a":0,"k":[1,0,0,1]}},"o":{{"a":0,"k":100}},"r":1}}{shapes_extra},
        {{"ty":"tr","p":{{"a":0,"k":[0,0]}},"a":{{"a":0,"k":[0,0]}},"s":{{"a":0,"k":[100,100]}},"r":{{"a":0,"k":0}},"o":{{"a":0,"k":100}}}}]}}]}}"#
    )
}

fn anim(layers: &str) -> String {
    format!(r#"{{{HEAD},"layers":[{layers}]}}"#)
}

#[test]
fn qizil_kvadrat_chiziladi() {
    let f = render(&anim(&rect_layer("", "")), 0.0);
    assert_eq!((f.width, f.height), (20, 20));
    assert_eq!(px(&f, 10, 10), [255, 0, 0, 255]);
    assert_eq!(px(&f, 2, 2), [0, 0, 0, 0]);
    assert_eq!(px(&f, 15, 10), [0, 0, 0, 0]);
}

#[test]
fn masshtab_kvadrat_chizilishini_ozgartiradi() {
    let f = Renderer::default().render_frame(&Animation::from_json_str(&anim(&rect_layer("", ""))).unwrap(), 0.0, RenderConfig::new(crate::Rgba8::TRANSPARENT, 2.0)).unwrap();
    assert_eq!((f.width, f.height), (40, 40));
    assert_eq!(px(&f, 20, 20), [255, 0, 0, 255]);
}

#[test]
fn kadr_oynasidan_tashqari_qatlam_korinmaydi() {
    let l = rect_layer("", "").replace(r#""ip":0,"op":30"#, r#""ip":10,"op":20"#);
    assert_eq!(px(&render(&anim(&l), 5.0), 10, 10)[3], 0);
    assert_eq!(px(&render(&anim(&l), 12.0), 10, 10)[3], 255);
}

#[test]
fn matn_qatlami_rad_etiladi() {
    let text = format!(r#"{{"ddd":0,"ind":1,"ty":5,"nm":"t","sr":1,{KS},"ao":0,"ip":0,"op":30,"st":0,"bm":0}}"#);
    let a = Animation::from_json_str(&anim(&text)).unwrap();
    match Renderer::default().render_frame(&a, 0.0, RenderConfig::default()) {
        Err(LottieError::UnsupportedFeatures { .. }) | Err(LottieError::InvalidTextLayer { .. }) => {}
        other => panic!("kutilmagan: {other:?}"),
    }
}

#[test]
fn maska_kesadi() {
    let mask = r#","hasMask":true,"masksProperties":[{"inv":false,"mode":"a","pt":{"a":0,"k":{"i":[[0,0],[0,0],[0,0],[0,0]],"o":[[0,0],[0,0],[0,0],[0,0]],"v":[[0,0],[10,0],[10,20],[0,20]],"c":true}},"o":{"a":0,"k":100},"x":{"a":0,"k":0}}]"#;
    let f = render(&anim(&rect_layer(mask, "")), 0.0);
    assert_eq!(px(&f, 7, 10), [255, 0, 0, 255]);
    assert_eq!(px(&f, 13, 10), [0, 0, 0, 0]);
}

#[test]
fn trim_chiziqni_qisqartiradi() {
    // UZ: 20x20 kanvasda gorizontal chiziq, trim 0..50%. EN: A horizontal line on a 20x20 canvas, trim 0..50%. RU: Горизонтальная линия на холсте 20x20, trim 0..50%.
    let line = format!(
        r#"{{"ddd":0,"ind":1,"ty":4,"nm":"l","sr":1,{KS},"ao":0,"ip":0,"op":30,"st":0,"bm":0,"shapes":[{{"ty":"gr","it":[
        {{"ty":"sh","ks":{{"a":0,"k":{{"i":[[0,0],[0,0]],"o":[[0,0],[0,0]],"v":[[0,10],[20,10]],"c":false}}}}}},
        {{"ty":"st","c":{{"a":0,"k":[0,0,1,1]}},"o":{{"a":0,"k":100}},"w":{{"a":0,"k":4}},"lc":1,"lj":1,"ml":4}},
        {{"ty":"tm","s":{{"a":0,"k":0}},"e":{{"a":0,"k":50}},"o":{{"a":0,"k":0}},"m":1}},
        {{"ty":"tr","p":{{"a":0,"k":[0,0]}},"a":{{"a":0,"k":[0,0]}},"s":{{"a":0,"k":[100,100]}},"r":{{"a":0,"k":0}},"o":{{"a":0,"k":100}}}}]}}]}}"#
    );
    let f = render(&anim(&line), 0.0);
    assert_eq!(px(&f, 5, 10), [0, 0, 255, 255]);
    assert_eq!(px(&f, 15, 10)[3], 0);
}

#[test]
fn chiqish_oldindan_kopaytirilgan() {
    // UZ: Yarim shaffof qizil: premultiplied R = A. EN: Half-transparent red: premultiplied R = A. RU: Полупрозрачный красный: premultiplied R = A.
    let l = rect_layer("", "").replacen(r#""o":{"a":0,"k":100}"#, r#""o":{"a":0,"k":50}"#, 1);
    let f = render(&anim(&l), 0.0);
    let p = px(&f, 10, 10);
    assert!(p[3] >= 126 && p[3] <= 129 && p[0] == p[3], "{p:?}");
}

#[test]
fn tgs_bayt_oqimi_ochiladi_va_chiziladi() {
    let json = anim(&rect_layer("", ""));
    let text = crate::tgs::to_json(json.as_bytes()).unwrap();
    assert_eq!(px(&render(&text, 0.0), 10, 10), [255, 0, 0, 255]);
}

#[test]
fn buzilgan_json_xato_beradi() {
    assert!(Animation::from_json_str("{").is_err());
    assert!(Animation::from_json_str(r#"{"layers":5}"#).is_err());
}

#[test]
fn nol_ulchamli_kanvas_xato() {
    let a = Animation::from_json_str(&anim(&rect_layer("", ""))).unwrap();
    assert!(Renderer::default().render_frame(&a, 0.0, RenderConfig::new(crate::Rgba8::TRANSPARENT, 0.0)).is_err());
}

#[test]
fn ajratilgan_pozitsiya_s_true_bilan_ishlaydi() {
    let json = format!(
        r#"{{{HEAD},"layers":[{{"ty":4,"ind":1,"ip":0,"op":30,"st":0,"ks":{{"o":{{"a":0,"k":100}},"r":{{"a":0,"k":0}},"p":{{"s":true,"x":{{"a":0,"k":10}},"y":{{"a":0,"k":0}}}},"a":{{"a":0,"k":[0,0,0]}},"s":{{"a":0,"k":[100,100,100]}}}},"shapes":[{{"ty":"gr","it":[{{"ty":"rc","p":{{"a":0,"k":[5,5]}},"s":{{"a":0,"k":[10,10]}},"r":{{"a":0,"k":0}}}},{{"ty":"fl","c":{{"a":0,"k":[1,0,0,1]}},"o":{{"a":0,"k":100}}}},{{"ty":"tr","p":{{"a":0,"k":[0,0]}},"a":{{"a":0,"k":[0,0]}},"s":{{"a":0,"k":[100,100]}},"r":{{"a":0,"k":0}},"o":{{"a":0,"k":100}}}}]}}]}}]}}"#
    );
    let f = render(&json, 0.0);
    assert_eq!(px(&f, 17, 5)[3], 255);
    assert_eq!(px(&f, 2, 5)[3], 0);
}
