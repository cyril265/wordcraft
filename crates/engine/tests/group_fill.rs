//! A panel built as a group whose rounded rectangle and text box take the group's fill
//! (`a:grpFill`) is drawn in that fill, as in Word, not in the colour of the members' style.
//! Word's colours come from PDFs Word for Mac exported from synthetic documents like this one.

use std::io::Write;

use wordcraft_doc::fill::Fill;
use wordcraft_doc::para::ShapeKind;
use wordcraft_doc::props::Rgb;
use wordcraft_engine::Session;
use wordcraft_engine::layout::Placed;

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup""#;

fn open(body: &str) -> Session {
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body>{body}</w:body></w:document>"#);
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in [("_rels/.rels", root), ("word/document.xml", doc.as_str())] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(text.as_bytes()).unwrap();
    }
    Session::new(wordcraft_engine::io::open_bytes("d.docx", &zw.finish().unwrap().into_inner()).unwrap())
}

#[test]
fn a_panel_whose_members_take_the_groups_fill_is_drawn_in_it() {
    let style = r#"<wps:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></wps:style>"#;
    let member = |id: u32, x: u32, w: u32, h: u32, prst: &str, style: &str, text: bool| {
        let (nv, txbx) = if text {
            (r#"<wps:cNvSpPr txBox="1"/>"#, "<wps:txbx><w:txbxContent><w:p><w:r><w:t>T</w:t></w:r></w:p></w:txbxContent></wps:txbx>")
        } else {
            ("<wps:cNvSpPr/>", "")
        };
        format!(
            r#"<wps:wsp><wps:cNvPr id="{id}" name="m{id}"/>{nv}<wps:spPr><a:xfrm><a:off x="{x}" y="0"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom><a:grpFill/><a:ln><a:noFill/></a:ln></wps:spPr>{style}{txbx}<wps:bodyPr/></wps:wsp>"#
        )
    };
    let members = member(2, 0, 3000, 1000, "roundRect", style, false) + &member(3, 1500, 1500, 500, "rect", "", true);
    let group = format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="1" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>1270000</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>1270000</wp:posOffset></wp:positionV><wp:extent cx="3810000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="1" name="G"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3810000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="3000" cy="1000"/></a:xfrm><a:solidFill><a:schemeClr val="bg1"><a:lumMod val="95000"/></a:schemeClr></a:solidFill></wpg:grpSpPr>{members}</wpg:wgp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    );
    let mut s = open(&format!("<w:p>{group}<w:r><w:t>ANCHOR</w:t></w:r></w:p>"));
    let l = s.layout();
    let fills: Vec<_> = l.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Placed::Shape { rect, kind: ShapeKind::RoundedRectangle | ShapeKind::TextBox, fill, .. } => {
                Some((rect.x, rect.y, rect.w, rect.h, fill.clone()))
            }
            _ => None,
        })
        .collect();
    // Word: both F2F2F2 (white at 95% luminance), the rounded rectangle over the whole group at
    // 100, 100, 300 × 100 and the text box over its right half.
    let grey = Some(Fill::solid(Rgb(0xF2, 0xF2, 0xF2)));
    assert_eq!(fills, [(100.0, 100.0, 300.0, 100.0, grey.clone()), (250.0, 100.0, 150.0, 50.0, grey)]);
}
