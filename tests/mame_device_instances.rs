use mame_coalesce::mame::{self, MachineSpecification, MameRecord};

#[test]
fn public_mame_reader_preserves_repeated_device_instances() -> Result<(), Box<dyn std::error::Error>>
{
    let xml = br#"<mame mameconfig="10">
        <machine name="multi-instance-device">
            <description>Multiple device instances</description>
            <device type="cartridge">
                <vendor-specific/>
                <instance name="cassette" briefname="Cassette"/>
                <extension name="cart"/>
                <ignored-child/>
                <instance name="floppy" briefname="Floppy"/>
            </device>
        </machine>
    </mame>"#;
    let parsed = mame::read_with::<_, mame_coalesce::Error>(
        xml,
        |_| Ok(Vec::new()),
        |machines, record| {
            if let MameRecord::Machine(machine) = record {
                machines.push(*machine);
            }
            Ok(())
        },
    )?
    .into_inner();

    let [machine] = parsed.as_slice() else {
        return Err("expected one parsed machine".into());
    };
    let device = machine
        .specification
        .iter()
        .find_map(|element| match &element.value {
            MachineSpecification::Device(device) => Some(device),
            _ => None,
        })
        .ok_or("parsed machine has no device specification")?;
    let instances = device
        .instances
        .iter()
        .map(|instance| (instance.name.as_str(), instance.brief_name.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(instances, [("cassette", "Cassette"), ("floppy", "Floppy")]);
    assert_eq!(
        device
            .instances
            .iter()
            .map(|instance| instance.source_order)
            .collect::<Vec<_>>(),
        [1, 4]
    );
    assert_eq!(
        device
            .extensions
            .iter()
            .map(|extension| extension.source_order)
            .collect::<Vec<_>>(),
        [2]
    );
    Ok(())
}
