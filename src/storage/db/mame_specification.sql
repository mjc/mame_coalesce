-- Native query projection. Callers provide requested_mame_specification_owners;
-- neither source facts nor the resulting wide projection are stored twice.
element_keys AS (
    SELECT occurrence.record_id AS set_id, sample.source_order AS element_order, 'sample' AS element_type, sample.source_line, sample.source_column,
           sample.occurrence_id AS sample_occurrence_id
    FROM requested_mame_specification_owners AS requested
    CROSS JOIN asset_occurrences AS occurrence CROSS JOIN mame_samples AS sample
    WHERE occurrence.record_id=requested.set_id AND sample.occurrence_id=occurrence.occurrence_id AND occurrence.claim_kind='mame_sample'
    UNION ALL
    SELECT native.set_id, native.element_order, 'chip', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_chips AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'display', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_displays AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'sound', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_sounds AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'input', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_inputs AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'port', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_ports AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'adjuster', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_adjusters AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'driver', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_drivers AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'feature', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_features AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'device', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_devices AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'slot', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_slots AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'softwarelist', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_software_lists AS native
    WHERE native.set_id=requested.set_id
    UNION ALL
    SELECT native.set_id, native.element_order, 'ramoption', native.source_line, native.source_column, NULL
    FROM requested_mame_specification_owners AS requested CROSS JOIN mame_machine_ram_options AS native
    WHERE native.set_id=requested.set_id
)
SELECT e.set_id, e.element_order, e.element_type,
       sample.name AS sample_name,
       chip.name AS chip_name, chip.tag AS chip_tag, chip.kind AS chip_type, chip.clock AS chip_clock,
       display.tag AS display_tag, display.kind AS display_type, display.rotation AS display_rotate,
       display.flip_x AS flipx,display.flip_x_specified AS flipx_specified, display.width AS display_width, display.height AS display_height,
       display.refresh AS display_refresh, display.pixel_clock AS display_pixclock,
       display.horizontal_total AS display_htotal, display.horizontal_blank_end AS display_hbend,
       display.horizontal_blank_start AS display_hbstart, display.vertical_total AS display_vtotal,
       display.vertical_blank_end AS display_vbend, display.vertical_blank_start AS display_vbstart,
       sound.channels AS sound_channels,
       input.service AS input_service,input.service_specified AS input_service_specified,
       input.tilt AS input_tilt,input.tilt_specified AS input_tilt_specified,input.players AS input_players,
       input.coins AS input_coins, port.tag AS port_tag,
       adjuster.name AS adjuster_name, adjuster.default_value AS adjuster_default,
       driver.status AS driver_status, driver.emulation AS driver_emulation,
       driver.cocktail AS driver_cocktail, driver.savestate AS driver_savestate,
       driver.requires_artwork AS driver_requiresartwork,driver.requires_artwork_specified AS driver_requiresartwork_specified,
       driver.unofficial AS driver_unofficial,driver.unofficial_specified AS driver_unofficial_specified,
       driver.no_sound_hardware AS driver_nosoundhardware,driver.no_sound_hardware_specified AS driver_nosoundhardware_specified,
       driver.incomplete AS driver_incomplete,driver.incomplete_specified AS driver_incomplete_specified,
       feature.kind AS feature_type, feature.status AS feature_status, feature.overall AS feature_overall,
       device.kind AS device_type, device.tag AS device_tag, device.fixed_image AS device_fixed_image,
       device.mandatory AS device_mandatory, device.interface AS device_interface,
       instance.name AS device_instance_name, instance.brief_name AS device_instance_briefname,
       instance.source_line AS device_instance_line, instance.source_column AS device_instance_column,
       slot.name AS slot_name, software_list.tag AS softwarelist_tag,
       software_list.name AS softwarelist_name, software_list.status AS softwarelist_status,
       software_list.filter AS softwarelist_filter, ram.name AS ramoption_name,
       ram.default_value AS ramoption_default, ram.text AS ramoption_text,
       e.source_line, e.source_column
FROM element_keys AS e
LEFT JOIN mame_samples AS sample ON sample.occurrence_id=e.sample_occurrence_id
LEFT JOIN mame_machine_chips AS chip USING (set_id, element_order)
LEFT JOIN mame_machine_displays AS display USING (set_id, element_order)
LEFT JOIN mame_machine_sounds AS sound USING (set_id, element_order)
LEFT JOIN mame_machine_inputs AS input USING (set_id, element_order)
LEFT JOIN mame_machine_ports AS port USING (set_id, element_order)
LEFT JOIN mame_machine_adjusters AS adjuster USING (set_id, element_order)
LEFT JOIN mame_machine_drivers AS driver USING (set_id, element_order)
LEFT JOIN mame_machine_features AS feature USING (set_id, element_order)
LEFT JOIN mame_machine_devices AS device USING (set_id, element_order)
LEFT JOIN mame_machine_device_instances AS instance USING (set_id, element_order)
LEFT JOIN mame_machine_slots AS slot USING (set_id, element_order)
LEFT JOIN mame_machine_software_lists AS software_list USING (set_id, element_order)
LEFT JOIN mame_machine_ram_options AS ram USING (set_id, element_order)
