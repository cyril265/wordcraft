//! A VML rectangle or text box filled with a picture (`v:fill type="frame"`) is drawn with the
//! picture where Word draws it, its fill colour under the picture and its outline and text over
//! it, and its text is edited by clicking into it. Word's numbers come from PDFs Word for Mac
//! exported from synthetic documents like these (A4, 72 pt margins, exact 17 pt lines, a
//! 200 × 100 px picture).

use std::io::Write;

use wordcraft_doc::StoryRef;
use wordcraft_doc::fill::Fill;
use wordcraft_doc::para::ShapeKind;
use wordcraft_doc::props::Rgb;
use wordcraft_engine::Session;
use wordcraft_engine::layout::Placed;
use wordcraft_geom::Rect;

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office""#;
const PAGE: &str = "mso-position-horizontal-relative:page;mso-position-vertical-relative:page";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CYAN: Rgb = Rgb(0, 0xFF, 0xFF);

/// An opaque fill in `color`.
fn solid(color: Rgb) -> Option<Fill> {
    Some(Fill::solid(color))
}
const TEXT_BOX_TYPE: &str = r#"<v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"/>"#;

/// A PNG's signature and header (colour type `colour`: 2 opaque, 6 with alpha) and an empty
/// image data chunk: enough to tell whether it may be transparent.
fn png(colour: u8) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&200u32.to_be_bytes());
    b.extend_from_slice(&100u32.to_be_bytes());
    b.extend_from_slice(&[8, colour, 0, 0, 0, 0, 0, 0, 0]);
    b.extend_from_slice(&0u32.to_be_bytes());
    b.extend_from_slice(b"IDATxxxx");
    b
}

/// A document with `body`, and `header` as its header when given; `img1` is `picture`.
fn package(body: &str, header: Option<&str>, picture: &[u8]) -> Vec<u8> {
    let head_ref = if header.is_some() { r#"<w:headerReference w:type="default" r:id="rIdH"/>"# } else { "" };
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body>{body}<w:sectPr>{head_ref}<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let root = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let image = format!(r#"<Relationship Id="img1" Type="{REL}/image" Target="media/img1.png"/>"#);
    let rels = |more: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{image}{more}</Relationships>"#
        )
    };
    let head_rel = format!(r#"<Relationship Id="rIdH" Type="{REL}/header" Target="header1.xml"/>"#);
    let doc_rels = rels(if header.is_some() { &head_rel } else { "" });
    let head = header.map(|h| format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr {NS}>{h}</w:hdr>"#));
    let head_rels = rels("");
    let mut parts: Vec<(&str, &[u8])> = vec![
        ("_rels/.rels", root.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("word/media/img1.png", picture),
    ];
    if let Some(h) = &head {
        parts.push(("word/header1.xml", h.as_bytes()));
        parts.push(("word/_rels/header1.xml.rels", head_rels.as_bytes()));
    }
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn open_with(body: &str, header: Option<&str>, picture: &[u8]) -> Session {
    Session::new(wordcraft_engine::io::open_bytes("d.docx", &package(body, header, picture)).unwrap())
}

fn open(body: &str) -> Session {
    open_with(body, None, &png(2))
}

/// The session saved as DOCX and opened again.
fn reopened(s: &Session) -> Session {
    let bytes = wordcraft_engine::io::save_bytes("d.docx", &s.doc).unwrap();
    Session::new(wordcraft_engine::io::open_bytes("d.docx", &bytes).unwrap())
}

fn para(text: &str, runs: &str) -> String {
    format!(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="340" w:lineRule="exact"/></w:pPr>{runs}<w:r><w:t>{text}</w:t></w:r></w:p>"#)
}

fn rect(style: &str, attrs: &str, fill: &str) -> String {
    format!(r#"<w:r><w:pict><v:rect style="position:absolute;{style};z-index:1" {attrs}>{fill}</v:rect></w:pict></w:r>"#)
}

fn text_box(style: &str, attrs: &str, text: &str) -> String {
    format!(
        r##"<w:r><w:pict>{TEXT_BOX_TYPE}<v:shape type="#_x0000_t202" style="position:absolute;{style};z-index:1;{PAGE}" {attrs}>{FRAME}<v:textbox><w:txbxContent><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r>"##
    )
}

/// What is drawn on page 1, in drawing order: pictures, shapes (fill and outline) and the first
/// line of each text, as rects.
#[derive(Debug, PartialEq)]
enum Drawn {
    Picture(Rect),
    Shape(Rect, Option<Fill>, Option<Rgb>, f32),
    Text(f32, f32),
}

fn drawn(s: &mut Session) -> Vec<Drawn> {
    let l = s.layout();
    l.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Placed::Image { rect, media, spin, .. } if media == "img1.png" => {
                assert!(spin.is_identity(), "the picture is upright: {spin:?}");
                Some(Drawn::Picture(*rect))
            }
            Placed::Shape { rect, kind: ShapeKind::Rectangle | ShapeKind::TextBox, fill, stroke, stroke_width, .. } => {
                Some(Drawn::Shape(*rect, fill.clone(), *stroke, if stroke.is_some() { *stroke_width } else { 0.0 }))
            }
            Placed::Lines { x, y, story: StoryRef::Part(_), .. } => Some(Drawn::Text(*x, *y)),
            _ => None,
        })
        .collect()
}

fn pictures(d: &[Drawn]) -> Vec<Rect> {
    d.iter().filter_map(|d| if let Drawn::Picture(r) = d { Some(*r) } else { None }).collect()
}

fn outlines(d: &[Drawn]) -> Vec<(Rect, Rgb, f32)> {
    d.iter().filter_map(|d| if let Drawn::Shape(r, None, Some(c), w) = d { Some((*r, *c, *w)) } else { None }).collect()
}

fn near(a: Rect, b: [f32; 4], tol: f32) -> bool {
    [a.x - b[0], a.y - b[1], a.w - b[2], a.h - b[3]].iter().all(|d| d.abs() <= tol)
}

const FRAME: &str = r#"<v:fill type="frame" o:title="pic" r:id="img1"/>"#;

#[test]
fn a_rectangle_filled_with_a_picture_is_drawn_with_it_under_its_outline() {
    let at = |y: u32| format!("margin-left:100pt;margin-top:{y}pt;width:200pt;height:100pt;{PAGE}");
    let runs = [
        rect(&at(100), "", FRAME),
        rect(&at(300), r#"strokecolor="red" strokeweight="4pt""#, FRAME),
        rect(&at(500), r#"stroked="f""#, FRAME),
        rect(&at(650), r#"filled="f""#, FRAME),
    ]
    .concat();
    let mut s = open(&para("ANCHOR", &runs));
    let d = drawn(&mut s);
    // Word: the picture over the shape's box; black 0.75 pt and red 4 pt outlines over it; none
    // for the shape that isn't filled.
    let images = pictures(&d);
    assert_eq!(images.len(), 3, "{images:?}");
    for (r, y) in images.iter().zip([100.0, 300.0, 500.0]) {
        assert!(near(*r, [100.0, y, 200.0, 100.0], 0.01), "{r:?}");
    }
    let lines: Vec<_> = outlines(&d).iter().map(|(r, c, w)| (r.y, *c, *w)).collect();
    assert_eq!(lines, [(100.0, Rgb::BLACK, 0.75), (300.0, Rgb(0xFF, 0, 0), 4.0), (650.0, Rgb::BLACK, 0.75)]);
}

#[test]
fn a_logo_placed_from_its_line_is_where_word_draws_it() {
    // A shape like a letterhead logo: placed from the column and the line, a white outline.
    let style = "margin-left:268.55pt;margin-top:-12.25pt;width:79.35pt;height:69.85pt;mso-position-horizontal-relative:text;mso-position-vertical-relative:line";
    let fill = r#"<v:fill type="frame" o:title="logo" recolor="t" r:id="img1"/><o:lock v:ext="edit" aspectratio="t"/>"#;
    let mut s = open(&(para("ANCHOR", &rect(style, r#"strokecolor="white [3212]""#, fill)) + &para("END", "")));
    let d = drawn(&mut s);
    // Word: 340.47, 59.67, 79.5 × 70.0 (the box 340.55, 59.75, 79.35 × 69.85 and 0.08 pt more on
    // each side).
    let images = pictures(&d);
    assert_eq!(images.len(), 1, "{images:?}");
    assert!(near(images[0], [340.47, 59.67, 79.5, 70.0], 0.16), "{:?}", images[0]);
    let o = outlines(&d);
    assert_eq!(o.len(), 1);
    assert_eq!(o[0].1, Rgb::WHITE);
}

#[test]
fn a_text_box_filled_with_a_picture_shows_its_text_over_it() {
    let style = "margin-left:100pt;margin-top:100pt;width:200pt;height:100pt";
    let mut s = open(&para("ANCHOR", &text_box(style, "", "TEXTINBOX")));
    let d = drawn(&mut s);
    // Word: the picture at 100, 100, 200 × 100, the outline and then the text 7.55 pt in from the
    // box's left edge (its 7.2 pt margin) over it.
    let want = [
        Drawn::Picture(Rect::new(100.0, 100.0, 200.0, 100.0)),
        Drawn::Shape(Rect::new(100.0, 100.0, 200.0, 100.0), None, Some(Rgb::BLACK), 0.75),
        Drawn::Text(107.2, 103.6),
    ];
    assert_eq!(d, want);
}

/// Word draws the shape's fill colour under the picture: a cyan rectangle and text box whose
/// picture has a transparent left half and an opaque red right half are cyan on the left, red on
/// the right, with the text box's blue outline and text over them (Word for Mac: cyan fill,
/// picture, blue outline at 100, 300, 200 × 100, then the text). The same after a save.
#[test]
fn the_fill_colour_shows_under_a_transparent_picture() {
    let at = |y: u32| format!("margin-left:100pt;margin-top:{y}pt;width:200pt;height:100pt");
    let runs = rect(&format!("{};{PAGE}", at(100)), r##"fillcolor="#00ffff""##, FRAME)
        + &text_box(&at(300), r##"fillcolor="#00ffff" strokecolor="blue""##, "TBTEXT");
    let mut s = open_with(&para("ANCHOR", &runs), None, &png(6));
    let box_at = |y: f32| Rect::new(100.0, y, 200.0, 100.0);
    let want = [
        Drawn::Shape(box_at(100.0), solid(CYAN), None, 0.0),
        Drawn::Picture(box_at(100.0)),
        Drawn::Shape(box_at(100.0), None, Some(Rgb::BLACK), 0.75),
        Drawn::Shape(box_at(300.0), solid(CYAN), None, 0.0),
        Drawn::Picture(box_at(300.0)),
        Drawn::Shape(box_at(300.0), None, Some(Rgb(0, 0, 0xFF)), 0.75),
        Drawn::Text(107.2, 303.6),
    ];
    assert_eq!(drawn(&mut s), want);
    assert_eq!(drawn(&mut reopened(&s)), want, "saved");
}

/// The members of a `v:group` filled with a picture (Word for Mac: a 300 × 200 pt group at
/// 100, 100 with `coordsize` 600,400): the pictures at 100, 100, 200 × 100 under a red 2 pt
/// outline; at 250, 175, 50 × 150, upright, for a 150 × 50 member turned 90°; at 100, 225,
/// 80 × 50 over cyan; and at 310, 100, 90 × 100 for a text box in a nested group, over cyan,
/// under its blue outline and its text.
#[test]
fn group_members_filled_with_a_picture_are_where_word_draws_them() {
    let fill2 = r#"<v:fill type="frame" o:title="pic" r:id="img1"/>"#;
    let member =
        |el: &str, style: &str, attrs: &str, inner: &str| format!(r#"<{el} style="position:absolute;{style}" {attrs}>{fill2}{inner}</{el}>"#);
    let group = [
        format!(r#"<v:group style="position:absolute;margin-left:100pt;margin-top:100pt;width:300pt;height:200pt;z-index:1;{PAGE}" coordorigin="0,0" coordsize="600,400">"#),
        member("v:rect", "left:0;top:0;width:400;height:200", r##"fillcolor="#00ffff" strokecolor="red" strokeweight="2pt""##, ""),
        member("v:rect", "left:200;top:250;width:300;height:100;rotation:90", r#"stroked="f""#, ""),
        member("v:rect", "left:0;top:250;width:160;height:100", r##"fillcolor="#00ffff" stroked="f""##, ""),
        r#"<v:group style="position:absolute;left:420;top:0;width:180;height:200" coordorigin="0,0" coordsize="180,200">"#.to_string(),
        member(
            "v:shape",
            "left:0;top:0;width:180;height:200",
            r##"type="#_x0000_t202" fillcolor="#00ffff" strokecolor="blue""##,
            "<v:textbox><w:txbxContent><w:p><w:r><w:t>GRPTEXT</w:t></w:r></w:p></w:txbxContent></v:textbox>",
        ),
        "</v:group></v:group>".to_string(),
    ]
    .concat();
    let mut s = open_with(&para("ANCHOR", &format!("<w:r><w:pict>{TEXT_BOX_TYPE}{group}</w:pict></w:r>")), None, &png(6));
    let r = Rect::new;
    let white = solid(Rgb::WHITE);
    let want = [
        Drawn::Shape(r(100.0, 100.0, 200.0, 100.0), solid(CYAN), None, 0.0),
        Drawn::Picture(r(100.0, 100.0, 200.0, 100.0)),
        Drawn::Shape(r(100.0, 100.0, 200.0, 100.0), None, Some(Rgb(0xFF, 0, 0)), 2.0),
        Drawn::Shape(r(200.0, 225.0, 150.0, 50.0), white, None, 0.0),
        Drawn::Picture(r(250.0, 175.0, 50.0, 150.0)),
        Drawn::Shape(r(100.0, 225.0, 80.0, 50.0), solid(CYAN), None, 0.0),
        Drawn::Picture(r(100.0, 225.0, 80.0, 50.0)),
        Drawn::Shape(r(310.0, 100.0, 90.0, 100.0), solid(CYAN), None, 0.0),
        Drawn::Picture(r(310.0, 100.0, 90.0, 100.0)),
        Drawn::Shape(r(310.0, 100.0, 90.0, 100.0), None, Some(Rgb(0, 0, 0xFF)), 0.75),
        Drawn::Text(317.2, 103.6),
    ];
    assert_eq!(drawn(&mut s), want);
}

/// A text box filled with a picture is edited like any text box: a click inside it goes into its
/// text, one on its border takes the box, in the body and in a header.
#[test]
fn clicking_inside_a_text_box_filled_with_a_picture_edits_its_text() {
    let style = "margin-left:100pt;margin-top:100pt;width:200pt;height:100pt";
    let shape = text_box(style, "", "TEXTINBOX");
    let mut s = open(&(para("ANCHOR", &shape) + &para("END", "")));
    let l = s.layout();
    let text = |id: u32| s.doc.plain_text(StoryRef::Part(id));
    let Some(StoryRef::Part(id)) = l.story_at(0, 120.0, 110.0) else { panic!("{:?}", l.story_at(0, 120.0, 110.0)) };
    assert_eq!(text(id), "TEXTINBOX");
    assert!(l.object_at(0, 120.0, 110.0, 3.0).is_none(), "inside: the text");
    let edge = l.object_at(0, 100.5, 150.0, 3.0).expect("on the border: the box");
    assert_eq!(edge.rect, Rect::new(100.0, 100.0, 200.0, 100.0));
    assert_eq!(edge.text_box, Some(id));
    let mut s = open_with(&para("BODY", ""), Some(&para("HEADER", &shape)), &png(2));
    let l = s.layout();
    let head = StoryRef::Part(l.pages[0].header_story.expect("a header"));
    let Some(id) = l.header_footer_text_box_at(0, 120.0, 110.0) else { panic!("no text box in the header") };
    assert_eq!(s.doc.plain_text(StoryRef::Part(id)), "TEXTINBOX");
    assert!(l.object_at_in(0, 120.0, 110.0, 3.0, head).is_none(), "inside: the text");
    assert!(l.object_at_in(0, 100.5, 150.0, 3.0, head).is_some_and(|o| o.text_box == Some(id)), "on the border: the box");
}

/// A text box turned half round keeps its picture and its text upright (Word for Mac: the
/// picture at 100, 400, 200 × 100 as drawn unturned, the text at the box's top left).
#[test]
fn a_text_box_turned_half_round_keeps_its_picture_upright() {
    let style = "margin-left:100pt;margin-top:400pt;width:200pt;height:100pt;rotation:180";
    let mut s = open(&para("ANCHOR", &text_box(style, r##"fillcolor="#00ffff""##, "QTEXT")));
    let want = [
        Drawn::Picture(Rect::new(100.0, 400.0, 200.0, 100.0)),
        Drawn::Shape(Rect::new(100.0, 400.0, 200.0, 100.0), None, Some(Rgb::BLACK), 0.75),
        Drawn::Text(107.2, 403.6),
    ];
    assert_eq!(drawn(&mut s), want);
}
