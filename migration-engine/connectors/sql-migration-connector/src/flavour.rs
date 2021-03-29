//! SQL flavours implement behaviour specific to a given SQL implementation (PostgreSQL, SQLite...),
//! in order to avoid cluttering the connector with conditionals. This is a private implementation
//! detail of the SQL connector.

mod mssql;
mod mysql;
mod postgres;
mod sqlite;

pub(crate) use mssql::MssqlFlavour;
pub(crate) use mysql::MysqlFlavour;
pub(crate) use postgres::PostgresFlavour;
pub(crate) use sqlite::SqliteFlavour;

use crate::{
    connection_wrapper::Connection, sql_destructive_change_checker::DestructiveChangeCheckerFlavour,
    sql_renderer::SqlRenderer, sql_schema_calculator::SqlSchemaCalculatorFlavour,
    sql_schema_differ::SqlSchemaDifferFlavour, SqlMigrationConnector,
};
use datamodel::Datamodel;
use enumflags2::BitFlags;
use migration_connector::{ConnectorResult, MigrationDirectory, MigrationFeature};
use quaint::prelude::{ConnectionInfo, Table};
use sql_schema_describer::SqlSchema;
use std::fmt::Debug;

/// The maximum size of identifiers on MySQL, in bytes.
///
/// reference: https://dev.mysql.com/doc/refman/5.7/en/identifier-length.html
pub(crate) const MYSQL_IDENTIFIER_SIZE_LIMIT: usize = 64;

pub(crate) fn from_connection_info(
    connection_info: &ConnectionInfo,
    features: BitFlags<MigrationFeature>,
) -> Box<dyn SqlFlavour + Send + Sync + 'static> {
    match connection_info {
        ConnectionInfo::Mysql(url) => Box::new(MysqlFlavour::new(url.clone(), features)),
        ConnectionInfo::Postgres(url) => Box::new(PostgresFlavour::new(url.clone(), features)),
        ConnectionInfo::Sqlite { file_path, db_name } => Box::new(SqliteFlavour {
            file_path: file_path.clone(),
            attached_name: db_name.clone(),
            features,
        }),
        ConnectionInfo::Mssql(url) => Box::new(MssqlFlavour::new(url.clone(), features)),
        ConnectionInfo::InMemorySqlite { .. } => unreachable!("SqlFlavour for in-memory SQLite"),
    }
}

#[async_trait::async_trait]
pub(crate) trait SqlFlavour:
    DestructiveChangeCheckerFlavour + SqlRenderer + SqlSchemaDifferFlavour + SqlSchemaCalculatorFlavour + Debug
{
    async fn acquire_lock(&self, connection: &Connection) -> ConnectorResult<()>;

    fn check_database_version_compatibility(
        &self,
        _datamodel: &Datamodel,
    ) -> Option<user_facing_errors::common::DatabaseVersionIncompatibility> {
        None
    }

    /// Create a database for the given URL on the server, if applicable.
    async fn create_database(&self, database_url: &str) -> ConnectorResult<String>;

    /// Initialize the `_prisma_migrations` table.
    async fn create_migrations_table(&self, connection: &Connection) -> ConnectorResult<()>;

    /// Describe the SQL schema.
    async fn describe_schema<'a>(&'a self, conn: &Connection) -> ConnectorResult<SqlSchema>;

    /// Drop the database for the provided URL on the server.
    async fn drop_database(&self, database_url: &str) -> ConnectorResult<()>;

    /// Drop the migrations table
    async fn drop_migrations_table(&self, connection: &Connection) -> ConnectorResult<()>;

    /// Check a connection to make sure it is usable by the migration engine.
    /// This can include some set up on the database, like ensuring that the
    /// schema we connect to exists.
    async fn ensure_connection_validity(&self, connection: &Connection) -> ConnectorResult<()>;

    /// Perform the initialization required by connector-test-kit tests.
    async fn qe_setup(&self, database_url: &str) -> ConnectorResult<()>;

    /// Drop the database and recreate it empty.
    async fn reset(&self, connection: &Connection) -> ConnectorResult<()>;

    /// Optionally scan a migration script that could have been altered by users and emit warnings.
    fn scan_migration_script(&self, _script: &str) {}

    /// Apply the given migration history to a temporary database, and return
    /// the final introspected SQL schema.
    async fn sql_schema_from_migration_history(
        &self,
        migrations: &[MigrationDirectory],
        connection: &Connection,
        connector: &SqlMigrationConnector,
    ) -> ConnectorResult<SqlSchema>;

    /// Table to store applied migrations, the name part.
    fn migrations_table_name(&self) -> &'static str {
        "_prisma_migrations"
    }

    /// Table to store applied migrations.
    fn migrations_table(&self) -> Table<'_> {
        self.migrations_table_name().into()
    }

    /// Feature flags for the flavor
    fn features(&self) -> BitFlags<MigrationFeature>;
}
