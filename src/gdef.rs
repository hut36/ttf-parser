// https://docs.microsoft.com/en-us/typography/opentype/spec/gdef

use crate::{Font, GlyphId};
use crate::parser::{Stream, Offset, Offset16, Offset32, LazyArray16};
use crate::ggg::{Class, ClassDefinitionTable, CoverageTable};


/// A [glyph class](https://docs.microsoft.com/en-us/typography/opentype/spec/gdef#glyph-class-definition-table).
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(missing_docs)]
pub enum GlyphClass {
    Base      = 1,
    Ligature  = 2,
    Mark      = 3,
    Component = 4,
}


#[derive(Clone, Copy, Default)]
pub struct Table<'a> {
    glyph_classes: Option<ClassDefinitionTable<'a>>,
    mark_attach_classes: Option<ClassDefinitionTable<'a>>,
    mark_glyph_coverage_offsets: Option<(&'a [u8], LazyArray16<'a, Offset32>)>,
}

impl<'a> Table<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        let mut s = Stream::new(data);
        let version: u32 = s.read()?;
        if !(version == 0x00010000 || version == 0x00010002 || version == 0x00010003) {
            return None;
        }

        let glyph_class_def_offset: Option<Offset16> = s.read()?;
        s.skip::<Offset16>(); // attachListOffset
        s.skip::<Offset16>(); // ligCaretListOffset
        let mark_attach_class_def_offset: Option<Offset16> = s.read()?;

        let mut mark_glyph_sets_def_offset: Option<Offset16> = None;
        if version > 0x00010000 {
            mark_glyph_sets_def_offset = s.read()?;

            // version > 0x00010003
            // s.skip::<Offset32>(); // itemVarStoreOffset
        }

        let mut table = Table::default();

        if let Some(offset) = glyph_class_def_offset {
            if let Some(subdata) = data.get(offset.to_usize()..) {
                table.glyph_classes = Some(ClassDefinitionTable::new(subdata));
            }
        }

        if let Some(offset) = mark_attach_class_def_offset {
            if let Some(subdata) = data.get(offset.to_usize()..) {
                table.mark_attach_classes = Some(ClassDefinitionTable::new(subdata));
            }
        }

        if let Some(offset) = mark_glyph_sets_def_offset {
            if let Some(subdata) = data.get(offset.to_usize()..) {
                let mut s = Stream::new(subdata);
                let format: u16 = s.read()?;
                if format == 1 {
                    if let Some(array) = s.read_array16() {
                        table.mark_glyph_coverage_offsets = Some((subdata, array));
                    }
                }
            }
        }

        Some(table)
    }
}


impl<'a> Font<'a> {
    /// Checks that font has
    /// [Glyph Class Definition Table](https://docs.microsoft.com/en-us/typography/opentype/spec/gdef#glyph-class-definition-table).
    pub fn has_glyph_classes(&self) -> bool {
        self.gdef.map(|gdef| gdef.glyph_classes.is_some()).unwrap_or(false)
    }

    /// Parses glyph's class according to
    /// [Glyph Class Definition Table](https://docs.microsoft.com/en-us/typography/opentype/spec/gdef#glyph-class-definition-table).
    ///
    /// Returns `Ok(None)` when *Glyph Class Definition Table* is not set
    /// or glyph class is not set or invalid.
    pub fn glyph_class(&self, glyph_id: GlyphId) -> Option<GlyphClass> {
        match self.gdef?.glyph_classes?.get(glyph_id).0 {
            1 => Some(GlyphClass::Base),
            2 => Some(GlyphClass::Ligature),
            3 => Some(GlyphClass::Mark),
            4 => Some(GlyphClass::Component),
            _ => None,
        }
    }

    /// Parses glyph's mark attachment class according to
    /// [Mark Attachment Class Definition Table](https://docs.microsoft.com/en-us/typography/opentype/spec/gdef#mark-attachment-class-definition-table).
    ///
    /// All glyphs not assigned to a class fall into Class 0.
    pub fn glyph_mark_attachment_class(&self, glyph_id: GlyphId) -> Class {
        self.gdef.and_then(|gdef| gdef.mark_attach_classes)
            .map(|def| def.get(glyph_id))
            .unwrap_or(Class(0))
    }

    /// Checks that glyph is a mark according to
    /// [Mark Glyph Sets Table](https://docs.microsoft.com/en-us/typography/opentype/spec/gdef#mark-glyph-sets-table).
    ///
    /// `set_index` allows checking a specific glyph coverage set.
    /// Otherwise all sets will be checked.
    ///
    /// Returns `Ok(false)` when *Mark Glyph Sets Table* is not set.
    #[inline]
    pub fn is_mark_glyph(&self, glyph_id: GlyphId, set_index: Option<u16>) -> bool {
        is_mark_glyph_impl(self.gdef.as_ref(), glyph_id, set_index).is_some()
    }
}

#[inline(never)]
fn is_mark_glyph_impl(
    table: Option<&Table>,
    glyph_id: GlyphId,
    set_index: Option<u16>,
) -> Option<()> {
    let (data, offsets) = table?.mark_glyph_coverage_offsets?;

    if let Some(set_index) = set_index {
        if let Some(offset) = offsets.get(set_index) {
            let table = CoverageTable::new(data.get(offset.to_usize()..)?);
            if table.contains(glyph_id) {
                return Some(());
            }
        }
    } else {
        for offset in offsets {
            let table = CoverageTable::new(data.get(offset.to_usize()..)?);
            if table.contains(glyph_id) {
                return Some(());
            }
        }
    }

    None
}
