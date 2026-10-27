{% set mig_ts = ts | date(format="%Y%m%d_%H%M%S") -%}
{% set mig_name = name | snake_case -%}
{% set plural_snake = table | plural | snake_case -%}
{% set module_name = "m" ~  mig_ts ~ "_" ~ mig_name -%}
to: "migration/src/{{module_name}}.rs"
skip_glob: "migration/src/m????????_??????_{{mig_name}}.rs"
message: "Migration `{{mig_name}}` added! You can now apply it with `$ cargo loco db migrate && cargo loco db entities`."
injections:
- into: "migration/src/lib.rs"
  before: "inject-above"
  content: "            Box::new({{module_name}}::Migration),"
- into: "migration/src/lib.rs"
  before: "pub struct Migrator"
  content: "mod {{module_name}};"
---
use loco_kit_uuidv7_primary_key::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        {% for column in columns -%}
        add_column(m, "{{plural_snake}}", "{{column.0}}", ColType::{{column.1}}).await?;
        {% endfor -%}

        {% for ref in references -%}
        {% if ref.1 %}{% set ref_col = ref.1 %}{% else %}{% set ref_target = ref.0 | trim_end_matches(pat="?") | snake_case %}{% set ref_col = ref_target ~ "_id" %}{% endif -%}
        add_reference(m, "{{plural_snake}}", "{{ref.0}}", "{{ref_col}}").await?;
        {% endfor -%}

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        {% for ref in references -%}
        {% if ref.1 %}{% set ref_col = ref.1 %}{% else %}{% set ref_target = ref.0 | trim_end_matches(pat="?") | snake_case %}{% set ref_col = ref_target ~ "_id" %}{% endif -%}
        remove_reference(m, "{{plural_snake}}", "{{ref.0 | trim_end_matches(pat="?")}}", "{{ref_col}}").await?;
        remove_column(m, "{{plural_snake}}", "{{ref_col}}").await?;
        {% endfor -%}

        {% for column in columns -%}
        remove_column(m, "{{plural_snake}}", "{{column.0}}").await?;
        {% endfor -%}
        
        Ok(())
    }
}
