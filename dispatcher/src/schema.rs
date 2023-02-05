// @generated automatically by Diesel CLI.

diesel::table! {
    player (player_id) {
        player_id -> Int4,
        username -> Text,
        player_uuid -> Nullable<Uuid>,
    }
}

diesel::table! {
    player_scan (player_id, scan_id) {
        player_scan_uuid -> Uuid,
        player_id -> Int4,
        scan_id -> Int4,
    }
}

diesel::table! {
    scan (scan_id) {
        scan_id -> Int4,
        ip -> Inet,
        version -> Nullable<Text>,
        online_count -> Nullable<Int4>,
        max_count -> Nullable<Int4>,
        description -> Nullable<Text>,
        favicon -> Nullable<Text>,
    }
}

diesel::joinable!(player_scan -> player (player_id));
diesel::joinable!(player_scan -> scan (scan_id));

diesel::allow_tables_to_appear_in_same_query!(
    player,
    player_scan,
    scan,
);
