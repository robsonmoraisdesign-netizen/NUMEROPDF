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

#[test]
fn marker_pattern_renders_numbers_only_after_opt_in() {
    let mut d = Document::new_empty();
    pdfcraft_organize::insert_blank_page(&mut d, 0, 200.0, 200.0).unwrap();
    pdfcraft_edit::stamp(&mut d, 0, "MarkerTest", b"q 0.588235 0.117647 0.784314 RG 1 w 100 100 30 30 re S 100 100 m 130 130 l S 100 130 m 130 100 l S Q".to_vec()).unwrap();
    let p = Pdf::from_bytes(write_full(&d, &SaveOptions::default()).unwrap()).unwrap();
    let candidates = p
        .marker_candidates(0, &numeropdf::marker::Pattern::default())
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let config = Numbering {
        mode: Mode::PurpleMarker,
        first_page: 0,
        last_page: 0,
        ..Default::default()
    };
    assert!(p.numbered(&config).is_err());
    let result = p
        .numbered(&Numbering {
            allow_marker_cover: true,
            ..config
        })
        .unwrap();
    assert_eq!(result.candidates(0).unwrap()[0].text, "01/01");
    assert!(
        result
            .marker_candidates(0, &numeropdf::marker::Pattern::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn batch_continues_after_failure_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    let bad = dir.path().join("bad.pdf");
    fixture().save(&a).unwrap();
    fixture().extract(&[0, 1]).unwrap().save(&b).unwrap();
    std::fs::write(&bad, b"not pdf").unwrap();
    let inputs = vec![a.clone(), bad, b];
    let config = Numbering {
        start: 15,
        ..Default::default()
    };
    let mut progress = vec![];
    let results = numeropdf::batch::number_files(&inputs, dir.path(), &config, true, |n, total| {
        progress.push((n, total))
    });
    assert_eq!(progress, vec![(1, 3), (2, 3), (3, 3)]);
    assert!(results[1].error.is_some());
    let out = Pdf::open(results[2].output.as_ref().unwrap()).unwrap();
    assert_eq!(out.candidates(1).unwrap()[0].text, "15/01");
    assert_eq!(
        Pdf::open(&a).unwrap().candidates(1).unwrap()[0].text,
        "01/02"
    );
    assert!(
        numeropdf::batch::number_files(&inputs, dir.path(), &config, true, |_, _| {})
            .iter()
            .all(|r| r.error.is_some())
    );
}
#[test]
fn detects_cell_and_original_font_size() {
    let pdf = fixture();
    let c = pdf.candidates(1).unwrap().remove(0);
    assert!((c.area.width - 57.0).abs() < 0.01);
    assert!((c.area.height - 27.0).abs() < 0.01);
    assert_eq!(c.source_size, Some(14.0));
    let out = pdf
        .numbered(&Numbering {
            last_page: 2,
            size: 4.0,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.candidates(1).unwrap()[0].source_size, Some(14.0));
}
