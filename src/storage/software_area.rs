use crate::mame_softwarelist::AreaKind;

/// Recover the area kind only from exactly one detail with the actual owner ID.
pub(super) const fn native_kind(
    area_id: i64,
    data_id: Option<i64>,
    disk_id: Option<i64>,
) -> Option<AreaKind> {
    match (data_id, disk_id) {
        (Some(id), None) if id == area_id => Some(AreaKind::Data),
        (None, Some(id)) if id == area_id => Some(AreaKind::Disk),
        _ => None,
    }
}
