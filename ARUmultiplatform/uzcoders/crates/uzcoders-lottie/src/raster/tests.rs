//! UZ: Rasterizator testlari: qamrov aniqligi, chiziq, punktir, gradient, maska, aralashtirish.
//! EN: Rasterizer tests: coverage accuracy, strokes, dashes, gradients, masks, blending.
//! RU: Тесты растеризатора: точность покрытия, обводка, штрихи, градиенты, маски, смешивание.

use super::*;

fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * p.width() + x) * 4) as usize;
    [p.data()[i], p.data()[i + 1], p.data()[i + 2], p.data()[i + 3]]
}

fn white() -> Paint {
    let mut p = Paint::default();
    p.set_color_rgba8(255, 255, 255, 255);
    p
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    PathBuilder::from_rect(Rect::from_xywh(x, y, w, h).unwrap())
}

#[test]
fn butun_piksel_to_rtburchak_aniq() {
    let mut m = Pixmap::new(8, 8).unwrap();
    m.fill_path(&rect(2.0, 2.0, 4.0, 3.0), &white(), FillRule::Winding, Transform::identity(), None);
    assert_eq!(px(&m, 2, 2), [255; 4]);
    assert_eq!(px(&m, 5, 4), [255; 4]);
    assert_eq!(px(&m, 6, 4), [0; 4]);
    assert_eq!(px(&m, 1, 2), [0; 4]);
    assert_eq!(px(&m, 3, 5), [0; 4]);
}

#[test]
fn yarim_piksel_qirra_yarim_qamrov() {
    let mut m = Pixmap::new(8, 8).unwrap();
    m.fill_path(&rect(2.5, 2.0, 3.0, 2.0), &white(), FillRule::Winding, Transform::identity(), None);
    let a = px(&m, 2, 2)[3];
    assert!((i32::from(a) - 128).abs() <= 2, "{a}");
    assert_eq!(px(&m, 3, 2)[3], 255);
    let b = px(&m, 5, 2)[3];
    assert!((i32::from(b) - 128).abs() <= 2, "{b}");
}

#[test]
fn kanvas_tashqarisidagi_shakl_qirqiladi() {
    let mut m = Pixmap::new(4, 4).unwrap();
    m.fill_path(&rect(-10.0, -10.0, 30.0, 30.0), &white(), FillRule::Winding, Transform::identity(), None);
    assert!(m.data().chunks_exact(4).all(|p| p == [255; 4]));
    m.fill(Color::from_rgba8(0, 0, 0, 0));
    m.fill_path(&rect(100.0, 100.0, 5.0, 5.0), &white(), FillRule::Winding, Transform::identity(), None);
    assert!(m.data().iter().all(|v| *v == 0));
}

#[test]
fn ichki_kontur_teshik_va_juft_toq() {
    let mut b = PathBuilder::new();
    b.push_path(&rect(0.0, 0.0, 8.0, 8.0));
    // UZ: Teskari yo'nalishdagi ichki kontur — nolga teng bo'lmagan o'rashda ham teshik. EN: A reversed inner contour is a hole even for non-zero winding. RU: Обратный внутренний контур — дыра и при ненулевой намотке.
    b.move_to(2.0, 2.0);
    b.line_to(2.0, 6.0);
    b.line_to(6.0, 6.0);
    b.line_to(6.0, 2.0);
    b.close();
    let p = b.finish().unwrap();
    let mut m = Pixmap::new(8, 8).unwrap();
    m.fill_path(&p, &white(), FillRule::Winding, Transform::identity(), None);
    assert_eq!(px(&m, 1, 1), [255; 4]);
    assert_eq!(px(&m, 4, 4), [0; 4]);
    let mut e = Pixmap::new(8, 8).unwrap();
    let mut b2 = PathBuilder::new();
    b2.push_path(&rect(0.0, 0.0, 8.0, 8.0));
    b2.push_path(&rect(2.0, 2.0, 4.0, 4.0));
    e.fill_path(&b2.finish().unwrap(), &white(), FillRule::EvenOdd, Transform::identity(), None);
    assert_eq!(px(&e, 4, 4), [0; 4]);
    assert_eq!(px(&e, 1, 1), [255; 4]);
}

#[test]
fn almashtirish_masshtab_va_siljish() {
    let mut m = Pixmap::new(8, 8).unwrap();
    let t = Transform::identity().pre_translate(4.0, 4.0).pre_scale(2.0, 2.0);
    m.fill_path(&rect(0.0, 0.0, 1.0, 1.0), &white(), FillRule::Winding, t, None);
    assert_eq!(px(&m, 4, 4), [255; 4]);
    assert_eq!(px(&m, 5, 5), [255; 4]);
    assert_eq!(px(&m, 6, 6), [0; 4]);
    let inv = t.invert().unwrap();
    assert_eq!(inv.map(6.0, 6.0), (1.0, 1.0));
}

#[test]
fn chiziq_qalinligi_va_butt_uchi() {
    let mut m = Pixmap::new(12, 8).unwrap();
    let mut b = PathBuilder::new();
    b.move_to(2.0, 4.0);
    b.line_to(10.0, 4.0);
    let st = Stroke { width: 2.0, ..Stroke::default() };
    m.stroke_path(&b.finish().unwrap(), &white(), &st, Transform::identity(), None);
    assert_eq!(px(&m, 5, 3), [255; 4]);
    assert_eq!(px(&m, 5, 4), [255; 4]);
    assert_eq!(px(&m, 5, 2), [0; 4]);
    assert_eq!(px(&m, 5, 5), [0; 4]);
    assert_eq!(px(&m, 1, 4), [0; 4]);
    assert_eq!(px(&m, 10, 4), [0; 4]);
}

#[test]
fn yumaloq_va_kvadrat_uch_uzaytiradi() {
    for (cap, reach) in [(LineCap::Round, true), (LineCap::Square, true), (LineCap::Butt, false)] {
        let mut m = Pixmap::new(14, 8).unwrap();
        let mut b = PathBuilder::new();
        b.move_to(4.0, 4.0);
        b.line_to(10.0, 4.0);
        let st = Stroke { width: 4.0, line_cap: cap, ..Stroke::default() };
        m.stroke_path(&b.finish().unwrap(), &white(), &st, Transform::identity(), None);
        assert_eq!(px(&m, 3, 3)[3] > 200, reach, "{cap:?}");
        assert_eq!(px(&m, 11, 4)[3] > 200, reach, "{cap:?}");
    }
}

#[test]
fn nol_uzunlikli_punktir_yumaloq_uchda_nuqta() {
    let mut m = Pixmap::new(20, 8).unwrap();
    let mut b = PathBuilder::new();
    b.move_to(2.0, 4.0);
    b.line_to(18.0, 4.0);
    let dash = StrokeDash::new(vec![0.0, 6.0], 0.0).unwrap();
    let st = Stroke { width: 3.0, line_cap: LineCap::Round, dash: Some(dash), ..Stroke::default() };
    m.stroke_path(&b.finish().unwrap(), &white(), &st, Transform::identity(), None);
    // UZ: Nuqtalar x=2, 8, 14 da. EN: Dots at x=2, 8, 14. RU: Точки при x=2, 8, 14.
    for x in [2u32, 8, 14] {
        assert!(px(&m, x, 4)[3] > 200, "x={x}");
    }
    assert_eq!(px(&m, 5, 4)[3], 0);
    assert_eq!(px(&m, 11, 4)[3], 0);
}

#[test]
fn punktir_bolaklari_va_siljish() {
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.line_to(10.0, 0.0);
    let p = b.finish().unwrap();
    let d = p.dash(&StrokeDash::new(vec![2.0, 3.0], 0.0).unwrap(), 1.0).unwrap();
    let starts: Vec<f32> = d.segments().filter_map(|s| if let PathSegment::MoveTo(p) = s { Some(p.x) } else { None }).collect();
    assert_eq!(starts, vec![0.0, 5.0]);
    let d2 = p.dash(&StrokeDash::new(vec![2.0, 3.0], 1.0).unwrap(), 1.0).unwrap();
    let first = d2.segments().next().unwrap();
    assert!(matches!(first, PathSegment::MoveTo(p) if p.x.abs() < 1e-4));
}

#[test]
fn ingichka_chiziq_hairline_qamrovi() {
    let mut m = Pixmap::new(12, 8).unwrap();
    let mut b = PathBuilder::new();
    b.move_to(1.0, 4.5);
    b.line_to(11.0, 4.5);
    let st = Stroke { width: 0.5, ..Stroke::default() };
    m.stroke_path(&b.finish().unwrap(), &white(), &st, Transform::identity(), None);
    let a = px(&m, 6, 4)[3];
    assert!((i32::from(a) - 128).abs() <= 3, "{a}");
}

#[test]
fn chiziqli_gradient_chekkalari_va_ortasi() {
    let stops = vec![GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)), GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255))];
    let shader = LinearGradient::new(Point::from_xy(0.0, 0.0), Point::from_xy(10.0, 0.0), stops, SpreadMode::Pad, Transform::identity()).unwrap();
    let paint = Paint { shader, ..Paint::default() };
    let mut m = Pixmap::new(10, 2).unwrap();
    m.fill_path(&rect(0.0, 0.0, 10.0, 2.0), &paint, FillRule::Winding, Transform::identity(), None);
    let (l, r) = (px(&m, 0, 0), px(&m, 9, 0));
    assert!(l[0] > 220 && l[2] < 30, "{l:?}");
    assert!(r[2] > 220 && r[0] < 30, "{r:?}");
    let mid = px(&m, 5, 0);
    assert!((i32::from(mid[0]) - i32::from(mid[2])).abs() < 40, "{mid:?}");
}

#[test]
fn radial_gradient_markazdan_chetga() {
    let stops = vec![GradientStop::new(0.0, Color::from_rgba8(255, 255, 255, 255)), GradientStop::new(1.0, Color::from_rgba8(255, 255, 255, 0))];
    let c = Point::from_xy(10.0, 10.0);
    let shader = RadialGradient::new(c, 0.0, c, 10.0, stops, SpreadMode::Pad, Transform::identity()).unwrap();
    let paint = Paint { shader, ..Paint::default() };
    let mut m = Pixmap::new(20, 20).unwrap();
    m.fill_path(&rect(0.0, 0.0, 20.0, 20.0), &paint, FillRule::Winding, Transform::identity(), None);
    assert!(px(&m, 10, 10)[3] > 225);
    assert!(px(&m, 0, 10)[3] < 30);
    assert!(px(&m, 15, 10)[3] > px(&m, 18, 10)[3]);
}

#[test]
fn aralashtirish_rejimlari() {
    let mut dst = Pixmap::new(2, 1).unwrap();
    dst.fill(Color::from_rgba8(255, 255, 255, 255));
    let mut src = Pixmap::new(2, 1).unwrap();
    src.fill(Color::from_rgba8(255, 0, 0, 128));
    let mut over = dst.clone();
    over.draw_pixmap(0, 0, &src, &PixmapPaint::default(), Transform::identity(), None);
    let o = px(&over, 0, 0);
    assert!(o[0] == 255 && (i32::from(o[1]) - 127).abs() <= 2, "{o:?}");
    let mut out = dst.clone();
    out.draw_pixmap(0, 0, &src, &PixmapPaint { blend_mode: BlendMode::DestinationOut, ..PixmapPaint::default() }, Transform::identity(), None);
    assert!((i32::from(px(&out, 0, 0)[3]) - 127).abs() <= 2);
    let mut sin = dst.clone();
    sin.draw_pixmap(0, 0, &src, &PixmapPaint { blend_mode: BlendMode::SourceIn, ..PixmapPaint::default() }, Transform::identity(), None);
    assert!((i32::from(px(&sin, 0, 0)[3]) - 128).abs() <= 2);
    let mut half = Pixmap::new(2, 1).unwrap();
    half.draw_pixmap(0, 0, &src, &PixmapPaint { opacity: 0.5, ..PixmapPaint::default() }, Transform::identity(), None);
    assert!((i32::from(px(&half, 0, 0)[3]) - 64).abs() <= 2);
}

#[test]
fn maska_alfa_yorqinlik_va_teskari() {
    let mut p = Pixmap::new(2, 1).unwrap();
    p.fill(Color::from_rgba8(255, 255, 255, 255));
    let mut m = Mask::from_pixmap(&p, MaskType::Alpha);
    let mut q = Pixmap::new(2, 1).unwrap();
    q.fill(Color::from_rgba8(255, 255, 255, 255));
    q.apply_mask(&m);
    assert_eq!(px(&q, 0, 0), [255; 4]);
    m.invert();
    q.apply_mask(&m);
    assert_eq!(px(&q, 0, 0), [0; 4]);
    let mut black = Pixmap::new(1, 1).unwrap();
    black.fill(Color::from_rgba8(0, 0, 0, 255));
    let lm = Mask::from_pixmap(&black, MaskType::Luminance);
    let mut w = Pixmap::new(1, 1).unwrap();
    w.fill(Color::from_rgba8(255, 255, 255, 255));
    w.apply_mask(&lm);
    assert_eq!(px(&w, 0, 0), [0; 4]);
}

#[test]
fn pixmap_chegaralari_va_kesish() {
    assert!(Pixmap::new(0, 5).is_none());
    assert!(Pixmap::new(100_000, 100_000).is_none());
    let mut p = Pixmap::new(4, 4).unwrap();
    p.fill(Color::from_rgba8(10, 20, 30, 255));
    let c = p.clone_rect(IntRect::from_xywh(1, 1, 2, 2).unwrap()).unwrap();
    assert_eq!((c.width(), c.height()), (2, 2));
    assert!(p.clone_rect(IntRect::from_xywh(3, 3, 2, 2).unwrap()).is_none());
}

#[test]
fn transform_pre_concat_tartibi() {
    // UZ: pre_translate keyin pre_scale: avval masshtab, keyin siljish. EN: pre_translate then pre_scale: scale first, then translate. RU: pre_translate затем pre_scale: сначала масштаб, потом сдвиг.
    let t = Transform::identity().pre_translate(10.0, 0.0).pre_scale(2.0, 1.0);
    assert_eq!(t.map(1.0, 5.0), (12.0, 5.0));
    let r = Transform::identity().pre_rotate(90.0);
    let (x, y) = r.map(1.0, 0.0);
    assert!(x.abs() < 1e-5 && (y - 1.0).abs() < 1e-5);
}
