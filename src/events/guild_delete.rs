use crate::utils::guild_config::GuildConfig;
use serenity::{all::UnavailableGuild, client::Context, model::guild::Guild};

use log::{error, info, warn};

pub async fn db_delete(
    ctx: &Context,
    incomplete: &UnavailableGuild,
    _: &Option<Guild>,
    database: &mongodb::Client,
) {
    let guildid = incomplete.id.get();
    let gid_i64 = guildid as i64;

    let db_name = match std::env::var("BOT_DB_NAME") {
        Ok(v) => v,
        Err(e) => {
            error!("BOT_DB_NAME not set: {:?}", e);
            return;
        }
    };
    let collections: mongodb::Collection<bson::document::Document> =
        database.database("scene").collection(&db_name);

    if let Err(why) = collections
        .find_one_and_delete(mongodb::bson::doc! { "guild_id": gid_i64 }, None)
        .await
    {
        error!(
            "Couldn't delete DB record for guildid: {} {:?}",
            guildid, why
        );
    }

    match GuildConfig::delete(&incomplete.id, ctx).await {
        Ok(_) => info!("guild config on memory-hashmap has been deleted completely."),
        Err(_) => warn!("guild config on memory-hashmap doesn't exist."),
    };
}
