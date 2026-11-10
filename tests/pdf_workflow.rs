use numeropdf::{Area, Mode, Numbering, Pdf};
use pdfcraft_cos::{Document, SaveOptions, write_full};
use pdfcraft_edit::{AddedText, Align, Content, Family};
fn fixture() -> Pdf {
    let mut d = Document::new_empty();
    for page in 0..3 {
        pdfcraft_organize::insert_blank_page(&mut d, page, 842.0, 595.0).unwrap();
        if page > 0 {
            pdfcraft_edit::add_content(
                &mut d,
                page,
                &Content::Text(AddedText {
                    rect: [780.0, 40.0, 830.0, 60.0],
                    text: format!("0{page}/02"),
                    family: Family::Helvetica,
                    bold: true,
                    italic: false,
                    size: 14.0,
                    color: [0.0; 3],
                    ink: None,
                    align: Align::Center,
                    char_spacing: 0.0,
                    aligns: vec![],
                }),
            )
            .unwrap();
        }
        pdfcraft_edit::stamp(
            &mut d,
            page,
            "TestBorder",
            b"q 0 0 0 RG 775 35 60 30 re S Q".to_vec(),
        )
        .unwrap();
    }
    Pdf::from_bytes(write_full(&d, &SaveOptions::default()).unwrap()).unwrap()
}
#[test]
fn replace_reopens_and_removes_old_count() {
    let p = fixture();
    let c = Numbering {
        last_page: 2,
        start: 15,
        ..Default::default()
    };
    let out = p.numbered(&c).unwrap();
    assert!(out.candidates(0).unwrap().is_empty());
    assert_eq!(out.candidates(1).unwrap()[0].text, "15/02");
    assert_eq!(out.candidates(2).unwrap()[0].text, "16/02");
    assert_eq!(p.candidates(1).unwrap()[0].text, "01/02");
    assert_eq!(out.render(1, 1.0).unwrap().dimensions(), (842, 595));
}
#[test]
fn extract_reorders_and_merge_preserves_count() {
    let p = fixture();
    let reordered = p.extract(&[2, 0, 1]).unwrap();
    assert_eq!(reordered.candidates(0).unwrap()[0].text, "02/02");
    let single = p.extract(&[1]).unwrap();
    assert_eq!(single.merge(&single).unwrap().pages.len(), 2);
}
#[test]
fn missing_count_is_an_error_not_a_silent_skip() {
    let p = fixture();
    assert!(
        p.numbered(&Numbering {
            first_page: 0,
            last_page: 2,
            ..Default::default()
        })
        .is_err()
    );
}
#[test]
fn manual_numbering_on_cover() {
    let p = fixture();
    let c = Numbering {
        mode: Mode::Manual,
        first_page: 0,
        last_page: 0,
        area: Area {
            x: 700.0,
            y: 500.0,
            width: 60.0,
            height: 20.0,
        },
        ..Default::default()
    };
    let out = p.numbered(&c).unwrap();
    assert_eq!(out.candidates(0).unwrap()[0].text, "01/01");
}
#[test]
fn rejects_out_of_bounds_and_oversized_text() {
    let p = fixture();
    let c = Numbering {
        mode: Mode::Manual,
        first_page: 0,
        last_page: 0,
        area: Area {
            x: 841.0,
            y: 10.0,
            width: 50.0,
            height: 20.0,
        },
        ..Default::default()
    };
    assert!(p.numbered(&c).is_err());
    let c = Numbering {
        area: Area {
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 4.0,
        },
        auto_fit: false,
        ..c
    };
    assert!(p.numbered(&c).is_err());
}
#[test]
fn ambiguous_numbers_require_review() {
    let p = fixture();
    let c = Numbering {
        mode: Mode::Manual,
        first_page: 1,
        last_page: 1,
        area: Area {
            x: 700.0,
            y: 500.0,
            width: 60.0,
            height: 20.0,
        },
        ..Default::default()
    };
    let out = p.numbered(&c).unwrap();
    assert_eq!(out.candidates(1).unwrap().len(), 2);
    assert!(
        out.numbered(&Numbering {
            first_page: 1,
            last_page: 1,
            ..Default::default()
        })
        .is_err()
    );
}
#[test]
fn rotated_page_numbers_stay_inside_page() {
    let p = fixture();
    let mut d = Document::open(p.bytes.clone()).unwrap();
    pdfcraft_organize::rotate_pages(&mut d, &[1], 90).unwrap();
    let p = Pdf::from_bytes(write_full(&d, &SaveOptions::default()).unwrap()).unwrap();
    let out = p
        .numbered(&Numbering {
            first_page: 1,
            last_page: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.pages[1].rotation, 90);
    assert!(out.candidates(1).unwrap()[0].area.valid(&out.pages[1]));
}
