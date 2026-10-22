//! Page breaks at the end of the body or of a section, and the empty paragraph closing a
//! section after one. Expected pages measured in Word for Mac 16.114 on synthetic documents
//! (A4, one paragraph per line, compatibility modes 12 and 15 alike).
use super::*;
use wordcraft_doc::para::ShapeKind;
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{Pos, Table, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// One body block in a case: `text` (`\u{c}` a page break, `\u{e}` a column break), what it
/// carries.
#[derive(Clone, Copy)]
enum Item {
    Text(&'static str),
    /// A paragraph closing its section (next page, the default), with this text.
    Closing(&'static str),
    /// The same with page break before.
    ClosingBreakBefore,
    /// A bookmark after this text.
    Bookmarked(&'static str),
    /// The same, closing its section.
    ClosingBookmarked,
    /// A shape anchored to the paragraph, in front of the text, after this text.
    Shape(&'static str),
    /// The same, closing its section.
    ClosingShape,
    /// A one-cell table holding these paragraphs.
    Cell(&'static [&'static str]),
}

fn shape() -> InlineObject {
    InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w: 100.0,
        h: 30.0,
        fill: None,
        stroke: None,
        stroke_width: 1.0,
        float: Float { wrap: Wrap::InFrontOfText, v_rel: Anchor::Paragraph, ..Default::default() },
        story: None,
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    }
}

/// The body of `items`, its last section starting as `next`.
fn doc(items: &[Item], next: SectionStart) -> Document {
    let mut d = Document::from_text("");
    d.body = items
        .iter()
        .map(|it| match it {
            Item::Cell(texts) => {
                let mut t = Table::new(1, 1, 200.0);
                if let Some(c) = t.rows.first_mut().and_then(|r| r.cells.first_mut()) {
                    c.blocks = texts.iter().map(|s| para_block(Paragraph::with_text(s, Default::default()))).collect();
                }
                Arc::new(Block::Table(t))
            }
            Item::Text(s) | Item::Closing(s) | Item::Bookmarked(s) | Item::Shape(s) => para_block(Paragraph::with_text(s, Default::default())),
            Item::ClosingBreakBefore | Item::ClosingBookmarked | Item::ClosingShape => para_block(Paragraph::with_text("", Default::default())),
        })
        .collect();
    for (i, it) in items.iter().enumerate() {
        let end = d.para(StoryRef::Body, &Path::top(i)).map_or(0, |p| p.len());
        match it {
            Item::Bookmarked(_) | Item::ClosingBookmarked => {
                d.insert_object(&Pos::body(i, end), InlineObject::BookmarkEnd { name: "mark".into() }, &Default::default()).unwrap();
                d.insert_object(&Pos::body(i, end), InlineObject::BookmarkStart { name: "mark".into() }, &Default::default()).unwrap();
            }
            Item::Shape(_) | Item::ClosingShape => {
                d.insert_object(&Pos::body(i, end), shape(), &Default::default()).unwrap();
            }
            _ => {}
        }
        if let Ok(p) = d.para_mut(StoryRef::Body, &Path::top(i)) {
            if matches!(it, Item::Closing(_) | Item::ClosingBreakBefore | Item::ClosingBookmarked | Item::ClosingShape) {
                p.section = Some(Box::new(SectionProps::default()));
            }
            if matches!(it, Item::ClosingBreakBefore) {
                p.props.page_break_before = Some(true);
            }
        }
    }
    d.last_section.start = next;
    d
}

/// The page count, and the number of the page each block starts on.
fn pages(d: &Document) -> (usize, Vec<Option<u32>>) {
    let l = lay(d);
    let first = (0..u32::try_from(d.body.len()).unwrap())
        .map(|b| {
            l.pages
                .iter()
                .find(|pg| pg.items.iter().any(|it| matches!(it, Placed::Lines { story: StoryRef::Body, path, .. } if path.0.first() == Some(&b))))
                .map(|pg| pg.number)
        })
        .collect();
    (l.pages.len(), first)
}

use Item::*;

#[test]
fn a_page_break_ending_the_body_starts_no_page() {
    // (body, Word's page count): the break is the last thing in the body's last paragraph,
    // after text or alone, followed only by a bookmark; an empty paragraph after the break gets
    // the new page, and so does a space, a second break or a paragraph holding a bookmark or a
    // shape.
    let cases: &[(&[Item], usize)] = &[
        (&[Text("AAA"), Text("ZZZ\u{c}")], 1),
        (&[Text("AAA"), Text("\u{c}")], 1),
        (&[Text("AAA"), Bookmarked("\u{c}")], 1),
        (&[Text("AAA\u{c}"), Text("")], 2),
        (&[Text("AAA\u{c}"), Text(""), Text("")], 2),
        (&[Text("AAA"), Text("\u{c}"), Text("\u{c}")], 2),
        (&[Text("AAA"), Text("\u{c}\u{c}")], 2),
        (&[Text("AAA"), Text("\u{c} ")], 2),
        (&[Text("AAA\u{c}"), Bookmarked("")], 2),
        (&[Text("AAA\u{c}"), Shape("")], 2),
        (&[Text("AAA"), Text("ZZZ\u{e}")], 2),
        (&[Text("AAA"), Text("\u{e}")], 2),
        (&[Text("AAA"), Cell(&["CCC\u{c}"]), Text("")], 1),
        (&[Text("AAA"), Cell(&["CCC", "\u{c}"]), Text("")], 1),
    ];
    for (i, (items, want)) in cases.iter().enumerate() {
        let (n, firsts) = pages(&doc(items, SectionStart::NextPage));
        assert_eq!(n, *want, "case {i}: pages start at {firsts:?}");
    }
}

#[test]
fn an_empty_paragraph_closing_a_section_after_a_page_break_starts_no_page() {
    // (body, how the section after starts, Word's page count, the page number "BBB" is on): an
    // empty paragraph (bookmarks don't count) closing a section after a page break stays with
    // the break; the next section starts on the page after it, on the next even or odd page if
    // it asks for one. Text or a shape in the closing paragraph, an empty paragraph before it or
    // a column break instead of the page break start a page as usual.
    let cases: &[(&[Item], SectionStart, usize, u32)] = &[
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::NextPage, 2, 2),
        (&[Text("AAA\u{c}"), Closing(""), Text("BBB")], SectionStart::NextPage, 2, 2),
        (&[Text("AAA"), Text("\u{c}"), ClosingBookmarked, Text("BBB")], SectionStart::NextPage, 2, 2),
        (&[Text("AAA"), Bookmarked("\u{c}"), Closing(""), Text("BBB")], SectionStart::NextPage, 2, 2),
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::EvenPage, 2, 2),
        (&[Text("AAA\u{c}"), Text("CCC"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::OddPage, 3, 3),
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::OddPage, 3, 3),
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::Continuous, 2, 2),
        (&[Text("AAA"), Text("\u{c}"), Text(""), Closing(""), Text("BBB")], SectionStart::NextPage, 3, 3),
        (&[Text("AAA"), Text("\u{c}"), Closing("SSS"), Text("BBB")], SectionStart::NextPage, 3, 3),
        (&[Text("AAA"), Text("\u{c}"), ClosingShape, Text("BBB")], SectionStart::NextPage, 3, 3),
        (&[Text("AAA"), Text("\u{e}"), Closing(""), Text("BBB")], SectionStart::NextPage, 3, 3),
    ];
    for (i, (items, next, want, bbb)) in cases.iter().enumerate() {
        let (n, firsts) = pages(&doc(items, *next));
        assert_eq!((n, firsts.last().copied().flatten()), (*want, Some(*bbb)), "case {i}: pages start at {firsts:?}");
    }
}

#[test]
fn a_page_break_ending_a_section_starts_no_page() {
    // (how the section after starts, Word's page count, the page number "BBB" is on): a page
    // break in the paragraph closing its section starts no page of its own: a section starting
    // on a new page (or an even one) starts on the next, a continuous one goes on on the page.
    let cases = [(SectionStart::NextPage, 2, 2), (SectionStart::EvenPage, 2, 2), (SectionStart::Continuous, 1, 1)];
    for (next, want, bbb) in cases {
        let (n, firsts) = pages(&doc(&[Text("AAA"), Closing("\u{c}"), Text("BBB")], next));
        assert_eq!((n, firsts.last().copied().flatten()), (want, Some(bbb)), "{next:?}: pages start at {firsts:?}");
    }
}

#[test]
fn an_empty_paragraph_closing_a_section_ignores_its_page_break_before() {
    // (body, how the section after starts, Word's page count, the page number "BBB" is on); an
    // empty last paragraph that closes no section keeps its page break before.
    let cases: &[(&[Item], SectionStart, usize, u32)] = &[
        (&[Text("AAA"), ClosingBreakBefore, Text("BBB")], SectionStart::NextPage, 2, 2),
        (&[Text("AAA"), ClosingBreakBefore, Text("BBB")], SectionStart::EvenPage, 2, 2),
        (&[Text("AAA"), ClosingBreakBefore, Text("BBB")], SectionStart::Continuous, 1, 1),
        (&[Text("AAA"), Text("\u{c}"), ClosingBreakBefore, Text("BBB")], SectionStart::NextPage, 2, 2),
        (&[Text("AAA"), Text("\u{c}"), ClosingBreakBefore, Text("BBB")], SectionStart::Continuous, 2, 2),
    ];
    for (i, (items, next, want, bbb)) in cases.iter().enumerate() {
        let (n, firsts) = pages(&doc(items, *next));
        assert_eq!((n, firsts.last().copied().flatten()), (*want, Some(*bbb)), "case {i}: pages start at {firsts:?}");
    }
    let mut d = doc(&[Text("AAA"), Text("")], SectionStart::NextPage);
    d.para_mut(StoryRef::Body, &Path::top(1)).unwrap().props.page_break_before = Some(true);
    assert_eq!(pages(&d).0, 2);
}

#[test]
fn the_caret_after_a_final_page_break_stays_on_its_page() {
    // After the break, the caret (the paragraph's mark) is on the break's page, below it, where
    // Word shows the mark; the break is kept for saving.
    for text in ["ZZZ\u{c}", "\u{c}"] {
        let d = doc(&[Text("AAA"), Text(text)], SectionStart::NextPage);
        let l = lay(&d);
        let end = Pos::body(1, text.len());
        let (before, after) = (l.caret(&Pos::body(1, 0)).unwrap(), l.caret(&end).unwrap());
        assert_eq!((l.pages.len(), after.page), (1, 0), "{text:?}");
        assert!(after.top > before.top, "{text:?}: the caret after the break at {}, before it at {}", after.top, before.top);
        assert_eq!(d.para(StoryRef::Body, &Path::top(1)).unwrap().text, text);
    }
    let d = doc(&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::NextPage);
    assert_eq!(lay(&d).caret(&Pos::body(2, 0)).map(|c| c.page), Some(0));
}

/// `items` with the blocks at `listed` in one numbered list, laid out: the page count, the
/// labels drawn with the page each is on, and the top of the last block on its page.
fn numbered(items: &[Item], next: SectionStart, listed: &[usize]) -> (usize, Vec<(usize, String)>, Option<f32>) {
    let mut d = doc(items, next);
    let id = d.numbering.add_list(wordcraft_doc::ListKind::Numbered);
    for &i in listed {
        d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().props.numbering = Some(wordcraft_doc::props::NumRef { num: id, level: 0 });
    }
    let l = lay(&d);
    let mut labels = Vec::new();
    let mut last_top = None;
    let last = u32::try_from(items.len().saturating_sub(1)).unwrap();
    for (i, pg) in l.pages.iter().enumerate() {
        for it in &pg.items {
            if let Placed::Lines { story: StoryRef::Body, path, para, l0, y, .. } = it {
                if *l0 == 0
                    && let Some(label) = &para.label
                {
                    labels.push((i, label.text.clone()));
                }
                if path.0.first() == Some(&last) {
                    last_top = Some(*y - pg.body.y);
                }
            }
        }
    }
    (l.pages.len(), labels, last_top)
}

#[test]
fn an_empty_paragraph_closing_a_section_has_no_list_label() {
    // Word draws no label on an empty numbered paragraph that closes a section, and it doesn't
    // count: after a page break, with page break before, in the middle of a page, before a
    // continuous section (taking no room: BBB right under AAA), alone in its section; the next
    // item is 2 (3 when a numbered break paragraph, which counts, comes first). An empty
    // numbered paragraph that closes no section keeps its label.
    let one = |p: usize| vec![(p, "1.".to_string())];
    let cases: Vec<(&[Item], SectionStart, &[usize], usize, Vec<(usize, String)>)> = vec![
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::NextPage, &[0, 1, 2], 2, one(0)),
        (&[Text("AAA\u{c}"), Closing(""), Text("BBB")], SectionStart::NextPage, &[0, 1], 2, one(0)),
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("BBB")], SectionStart::EvenPage, &[0, 1, 2], 2, one(0)),
        (&[Text("AAA"), ClosingBreakBefore, Text("BBB")], SectionStart::NextPage, &[0, 1], 2, one(0)),
        (&[Text("AAA"), Closing(""), Text("BBB")], SectionStart::NextPage, &[0, 1], 2, one(0)),
        (&[Text("AAA"), Closing(""), Text("BBB")], SectionStart::Continuous, &[0, 1], 1, one(0)),
        (&[Text("AAA"), ClosingBreakBefore, Text("BBB")], SectionStart::Continuous, &[0, 1], 1, one(0)),
        (&[Closing("AAA"), Closing(""), Text("BBB")], SectionStart::NextPage, &[0, 1], 3, one(0)),
        (&[Text("AAA"), Closing(""), Text("CCC")], SectionStart::NextPage, &[0, 1, 2], 2, vec![(0, "1.".into()), (1, "2.".into())]),
        (&[Text("AAA"), Text("\u{c}"), Closing(""), Text("CCC")], SectionStart::NextPage, &[0, 1, 2, 3], 2, vec![(0, "1.".into()), (1, "3.".into())]),
        (&[Text("AAA"), Text(""), Closing(""), Text("BBB")], SectionStart::NextPage, &[0, 1], 2, vec![(0, "1.".into()), (0, "2.".into())]),
        (&[Text("AAA\u{c}"), Text("")], SectionStart::NextPage, &[0, 1], 2, vec![(0, "1.".into()), (1, "2.".into())]),
    ];
    for (i, (items, next, listed, pages, labels)) in cases.iter().enumerate() {
        let (n, drawn, _) = numbered(items, *next, listed);
        assert_eq!((n, &drawn), (*pages, labels), "case {i}");
    }
    let bbb = |listed: &[usize]| numbered(&[Text("AAA"), Closing(""), Text("BBB")], SectionStart::Continuous, listed).2;
    assert_eq!(bbb(&[0, 1]), bbb(&[0]));
}
