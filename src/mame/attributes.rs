use crate::xml_reader::attribute_fields;

attribute_fields!(MameDocumentAttribute {
    Build = 0 => "build", Debug = 1 => "debug", MameConfig = 2 => "mameconfig",
});
attribute_fields!(MameMachineAttribute {
    Name = 0 => "name", SourceFile = 1 => "sourcefile", IsBios = 2 => "isbios",
    IsDevice = 3 => "isdevice", IsMechanical = 4 => "ismechanical", Runnable = 5 => "runnable",
    CloneOf = 6 => "cloneof", RomOf = 7 => "romof", SampleOf = 8 => "sampleof",
});
attribute_fields!(MameMachineCompatibilityAttribute { IsConsumable = 0 => "isconsumable" });
attribute_fields!(MameBiosAttribute { Name = 0 => "name", Description = 1 => "description", Default = 2 => "default" });
attribute_fields!(MameRomAttribute {
    Name = 0 => "name", Bios = 1 => "bios", Size = 2 => "size", Crc = 3 => "crc", Sha1 = 4 => "sha1",
    Merge = 5 => "merge", Region = 6 => "region", Offset = 7 => "offset", Status = 8 => "status", Optional = 9 => "optional",
});
attribute_fields!(MameRomCompatibilityAttribute {
    Md5 = 0 => "md5", SoundOnly = 1 => "soundonly", Dispose = 2 => "dispose", LoadFlag = 3 => "loadflag",
    Value = 4 => "value", Inverted = 5 => "inverted", Ovha = 6 => "ovha", NoThread = 7 => "nothread",
});
attribute_fields!(MameDiskAttribute {
    Name = 0 => "name", Sha1 = 1 => "sha1", Merge = 2 => "merge", Region = 3 => "region", Index = 4 => "index",
    Writable = 5 => "writable", Status = 6 => "status", Optional = 7 => "optional",
});
attribute_fields!(MameDiskCompatibilityAttribute { Writeable = 0 => "writeable" });
attribute_fields!(MameDeviceReferenceAttribute { Tag = 0 => "tag", Name = 1 => "name" });
attribute_fields!(MameSampleAttribute { Name = 0 => "name" });
attribute_fields!(MameChipAttribute { Name = 0 => "name", Tag = 1 => "tag", Type = 2 => "type", Clock = 3 => "clock" });
attribute_fields!(MameDisplayAttribute {
    Tag = 0 => "tag", Type = 1 => "type", Rotate = 2 => "rotate", FlipX = 3 => "flipx", Width = 4 => "width",
    Height = 5 => "height", Refresh = 6 => "refresh", Pixclock = 7 => "pixclock", Htotal = 8 => "htotal",
    Hbend = 9 => "hbend", Hbstart = 10 => "hbstart", Vtotal = 11 => "vtotal", Vbend = 12 => "vbend", Vbstart = 13 => "vbstart",
});
attribute_fields!(MameSoundAttribute { Channels = 0 => "channels" });
attribute_fields!(MameInputAttribute { Service = 0 => "service", Tilt = 1 => "tilt", Players = 2 => "players", Coins = 3 => "coins" });
attribute_fields!(MameControlAttribute {
    Type = 0 => "type", Player = 1 => "player", Buttons = 2 => "buttons", Minimum = 3 => "minimum", Maximum = 4 => "maximum",
    Sensitivity = 5 => "sensitivity", KeyDelta = 6 => "keydelta", Reverse = 7 => "reverse", Ways = 8 => "ways", Ways2 = 9 => "ways2", Ways3 = 10 => "ways3",
});
attribute_fields!(MameSwitchAttribute { Name = 0 => "name", Tag = 1 => "tag", Mask = 2 => "mask" });
attribute_fields!(MameSwitchLocationAttribute { Name = 0 => "name", Number = 1 => "number", Inverted = 2 => "inverted" });
attribute_fields!(MameSwitchValueAttribute { Name = 0 => "name", Value = 1 => "value", Default = 2 => "default" });
attribute_fields!(MameConditionAttribute { Tag = 0 => "tag", Mask = 1 => "mask", Relation = 2 => "relation", Value = 3 => "value" });
attribute_fields!(MamePortAttribute { Tag = 0 => "tag" });
attribute_fields!(MameAnalogAttribute { Mask = 0 => "mask" });
attribute_fields!(MameAdjusterAttribute { Name = 0 => "name", Default = 1 => "default" });
attribute_fields!(MameDriverAttribute {
    Status = 0 => "status", Emulation = 1 => "emulation", Cocktail = 2 => "cocktail", Savestate = 3 => "savestate",
    RequiresArtwork = 4 => "requiresartwork", Unofficial = 5 => "unofficial", NoSoundHardware = 6 => "nosoundhardware", Incomplete = 7 => "incomplete",
});
attribute_fields!(MameFeatureAttribute { Type = 0 => "type", Status = 1 => "status", Overall = 2 => "overall" });
attribute_fields!(MameDeviceAttribute { Type = 0 => "type", Tag = 1 => "tag", FixedImage = 2 => "fixed_image", Mandatory = 3 => "mandatory", Interface = 4 => "interface" });
attribute_fields!(MameInstanceAttribute { Name = 0 => "name", BriefName = 1 => "briefname" });
attribute_fields!(MameExtensionAttribute { Name = 0 => "name" });
attribute_fields!(MameSlotAttribute { Name = 0 => "name" });
attribute_fields!(MameSlotOptionAttribute { Name = 0 => "name", DevName = 1 => "devname", Default = 2 => "default" });
attribute_fields!(MameSoftwareListAttribute { Tag = 0 => "tag", Name = 1 => "name", Status = 2 => "status", Filter = 3 => "filter" });
attribute_fields!(MameRamOptionAttribute { Name = 0 => "name", Default = 1 => "default" });

pub type MameAttributePosition<Field> = crate::xml_reader::AttributePosition<Field>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MameAssetAttributePositions {
    #[default]
    None,
    Rom {
        native: Vec<MameAttributePosition<MameRomAttribute>>,
        compatibility: Vec<MameAttributePosition<MameRomCompatibilityAttribute>>,
    },
    Disk {
        native: Vec<MameAttributePosition<MameDiskAttribute>>,
        compatibility: Vec<MameAttributePosition<MameDiskCompatibilityAttribute>>,
    },
}

pub(super) fn select<F: Copy>(
    attributes: &crate::xml_reader::XmlAttributes,
    field: impl Fn(&str) -> Option<F>,
) -> crate::Result<Vec<crate::xml_reader::AttributePosition<F>>> {
    attributes
        .positions(field)
        .map(|position| {
            i64::try_from(position.source_order).map_err(|_| {
                crate::Error::InvalidPath("MAME attribute order exceeds SQLite INTEGER".into())
            })?;
            if position.location.line <= 0 || position.location.column <= 0 {
                return Err(crate::Error::InvalidPath(
                    "MAME attribute location must be positive".into(),
                ));
            }
            Ok(position)
        })
        .collect()
}
