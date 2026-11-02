//! Private-data check of the font table loader against the owner's image (skipped when absent).
//! Only structure is asserted: no metric value is stored in the repository.

use tsfp_ui_builders::image::find_image;
use tsfp_ui_model::font::{FontSet, FontTableMeasure, GLYPHS, KERN_SIZE, StringSource};
use tsfp_ui_model::layout::TextMeasure;
use tsfp_ui_model::{TextId, TextRef};

struct NoStrings;
impl StringSource for NoStrings {
    fn bytes(&self, _: TextId) -> Option<&[u8]> {
        None
    }
}

#[test]
fn loads_two_records_and_the_font_objects_from_the_image() {
    let Some(image) = find_image() else {
        eprintln!("skipped: no default.xbe");
        return;
    };
    let set = FontSet::load(&image);
    assert_eq!(set.records.len(), 2, "records 0 and 1 are populated");
    for record in set.records.values() {
        assert_eq!(record.glyphs.len(), GLYPHS);
        assert_eq!(record.kerning.len(), KERN_SIZE * KERN_SIZE);
        assert!(record.kerning.iter().all(|k| (-6..=0).contains(k)));
        assert!(
            record
                .glyphs
                .iter()
                .all(|g| g.kern_row < 13 && g.kern_column < 13)
        );
    }
    assert!(set.objects.len() >= 28, "objects: {}", set.objects.len());
    let strings = NoStrings;
    let measure = FontTableMeasure::new(&set, &strings);
    for font in [3u32, 4, 17] {
        assert!(measure.line_height(font) > 0.0);
    }
    let width = measure.width_px(4, b"Hello World");
    assert!(width > 0.0);
    assert_eq!(measure.text_width(4, TextRef::Id(TextId(1))), 0.0);
}
