/// Complete bundled schema for direct creation, never an upgrade script.
pub const SCHEMA: &str = concat!(
    include_str!("coverage.sql"),
    "\n",
    include_str!("schema.sql"),
    "\n",
    include_str!("logiqx.sql"),
    "\n",
    include_str!("cmp.sql"),
    "\n",
    include_str!("catalog_registry.sql"),
    "\n",
    include_str!("file_match_reviews.sql"),
    "\n",
    include_str!("no_intro_dat.sql"),
    "\n",
    include_str!("no_intro_dat_guards.sql"),
    "\n",
    include_str!("no_intro_database.sql"),
    "\n",
    include_str!("no_intro_database_guards.sql"),
    "\n",
    include_str!("software_guards.sql")
);
