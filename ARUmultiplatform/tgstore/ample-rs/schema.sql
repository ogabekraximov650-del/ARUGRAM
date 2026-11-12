CREATE TABLE IF NOT EXISTS anime_db (
            id INTEGER PRIMARY KEY,
            photo_url TEXT, name TEXT, davlat TEXT, studiya TEXT,
            janri TEXT, tavsif TEXT,
            created_at INTEGER DEFAULT 0
        );

CREATE TABLE IF NOT EXISTS season_db (
            anime_id INTEGER, season_id INTEGER, bolim_id INTEGER,
            photo_url TEXT, nomi TEXT, studio TEXT, tarjimon TEXT,
            yili TEXT, janri TEXT, turi TEXT, holati TEXT, tavsif TEXT,
            epizod_count INTEGER DEFAULT 0,
            views_total INTEGER DEFAULT 0,
            watch_ms_total INTEGER DEFAULT 0,
            fav_count INTEGER DEFAULT 0,
            rating_sum INTEGER DEFAULT 0,
            rating_count INTEGER DEFAULT 0,
            
            
            
            
            
            yosh INTEGER DEFAULT 0,
            created_at INTEGER,
            
            
            last_ep_at INTEGER DEFAULT 0,
            PRIMARY KEY (anime_id, season_id)
        );

CREATE TABLE IF NOT EXISTS epizod_db (
            anime_id INTEGER, season_id INTEGER, epizod_id INTEGER,
            epizod_number INTEGER, epizod_name TEXT,
            url_360p TEXT, size_360p INTEGER DEFAULT 0, key_360p TEXT DEFAULT '',
            url_480p TEXT, size_480p INTEGER DEFAULT 0, key_480p TEXT DEFAULT '',
            url_720p TEXT, size_720p INTEGER DEFAULT 0, key_720p TEXT DEFAULT '',
            url_1080p TEXT, size_1080p INTEGER DEFAULT 0, key_1080p TEXT DEFAULT '',
            intro_1 TEXT, intro_2 TEXT, intro_3 TEXT, intro_4 TEXT,
            intro_5 TEXT, intro_6 TEXT, intro_7 TEXT, intro_8 TEXT,
            intro_9 TEXT, intro_10 TEXT,
            views_total INTEGER DEFAULT 0,
            watch_ms_total INTEGER DEFAULT 0,
            created_at INTEGER,
            
            
            ready_at INTEGER DEFAULT 0,
            PRIMARY KEY (anime_id, season_id, epizod_id)
        );

CREATE TABLE IF NOT EXISTS season_janr (
            anime_id INTEGER, season_id INTEGER,
            janr_1 TEXT, janr_2 TEXT, janr_3 TEXT, janr_4 TEXT, janr_5 TEXT,
            janr_6 TEXT, janr_7 TEXT, janr_8 TEXT, janr_9 TEXT, janr_10 TEXT,
            janr_11 TEXT, janr_12 TEXT, janr_13 TEXT, janr_14 TEXT, janr_15 TEXT,
            janr_16 TEXT, janr_17 TEXT, janr_18 TEXT, janr_19 TEXT, janr_20 TEXT,
            PRIMARY KEY (anime_id, season_id)
        );

CREATE TABLE IF NOT EXISTS users_db (
            id INTEGER PRIMARY KEY,
            telegram_id INTEGER UNIQUE,
            username TEXT, first_name TEXT, last_name TEXT,
            is_banned INTEGER DEFAULT 0,
            balance INTEGER DEFAULT 0,
            avatar_file TEXT,
            profile_done INTEGER DEFAULT 1,
            traffic_bytes INTEGER DEFAULT 0,
            created_at INTEGER,
            last_login_at INTEGER,
            
            hidden_stats TEXT DEFAULT '',
            
            chan_consent INTEGER DEFAULT 0,
            
            ban_until INTEGER DEFAULT 0,
            ban_reason TEXT DEFAULT ''
        );

CREATE TABLE IF NOT EXISTS login_tokens (
            token TEXT PRIMARY KEY,
            status TEXT,
            user_id INTEGER,
            session_token TEXT,
            device TEXT, platform TEXT, app_version TEXT,
            created_at INTEGER,
            expires_at INTEGER
        );

CREATE TABLE IF NOT EXISTS sessions_db (
            id INTEGER PRIMARY KEY,
            user_id INTEGER,
            session_token TEXT UNIQUE,
            device TEXT,
            platform TEXT,
            app_version TEXT,
            created_at INTEGER,
            last_seen_at INTEGER
        );

CREATE TABLE IF NOT EXISTS watch_history_db (
            user_id INTEGER,
            anime_id INTEGER,
            season_id INTEGER,
            epizod_id INTEGER,
            video_url TEXT,
            last_quality TEXT,
            position_ms INTEGER,
            duration_ms INTEGER,
            watched_ms INTEGER DEFAULT 0,
            view_count INTEGER DEFAULT 0,
            deleted_at INTEGER DEFAULT 0,
            updated_at INTEGER,
            PRIMARY KEY (user_id, anime_id, season_id, epizod_id)
        );

CREATE TABLE IF NOT EXISTS ratings_db (
            user_id INTEGER, anime_id INTEGER, season_id INTEGER,
            stars INTEGER,
            updated_at INTEGER,
            PRIMARY KEY (user_id, anime_id, season_id)
        );

CREATE TABLE IF NOT EXISTS favorites_db (
            user_id INTEGER, anime_id INTEGER, season_id INTEGER,
            created_at INTEGER,
            PRIMARY KEY (user_id, anime_id, season_id)
        );

CREATE TABLE IF NOT EXISTS stats_hourly (
            hour TEXT, metric TEXT, value INTEGER,
            PRIMARY KEY (hour, metric)
        );

CREATE TABLE IF NOT EXISTS stats_daily (
            day TEXT, metric TEXT, value INTEGER,
            PRIMARY KEY (day, metric)
        );

CREATE TABLE IF NOT EXISTS sync_batches (
            user_id INTEGER PRIMARY KEY,
            batch_id TEXT,
            at INTEGER
        );

CREATE TABLE IF NOT EXISTS app_config (
            cfg_key TEXT PRIMARY KEY,
            cfg_value TEXT
        );

CREATE TABLE IF NOT EXISTS payments_db (
            order_id TEXT PRIMARY KEY,
            user_id INTEGER,
            amount INTEGER,
            status TEXT,
            pay_url TEXT,
            created_at INTEGER,
            expires_at INTEGER
        );

CREATE TABLE IF NOT EXISTS subs_db (
            user_id INTEGER PRIMARY KEY,
            expires_at INTEGER
        );

CREATE TABLE IF NOT EXISTS billing_log (
            id TEXT PRIMARY KEY,
            user_id INTEGER,
            kind TEXT,
            amount INTEGER,
            days INTEGER,
            note TEXT,
            created_at INTEGER
        );

CREATE TABLE IF NOT EXISTS comments_db (
            id TEXT PRIMARY KEY,
            anime_id INTEGER, season_id INTEGER,
            user_id INTEGER,
            
            
            parent_id TEXT DEFAULT '',
            body TEXT,
            
            media_file TEXT DEFAULT '',
            
            media_type TEXT DEFAULT '',
            likes INTEGER DEFAULT 0,
            reply_count INTEGER DEFAULT 0,
            
            
            deleted INTEGER DEFAULT 0,
            created_at INTEGER
        );

CREATE TABLE IF NOT EXISTS comment_likes (
            comment_id TEXT, user_id INTEGER,
            PRIMARY KEY (comment_id, user_id)
        );

CREATE TABLE IF NOT EXISTS reports_db (
            id TEXT PRIMARY KEY,
            
            kind TEXT DEFAULT 'comment',
            target_id TEXT DEFAULT '',
            target_body TEXT DEFAULT '',
            target_user_id INTEGER DEFAULT 0,
            anime_id INTEGER DEFAULT 0,
            season_id INTEGER DEFAULT 0,
            reporter_id INTEGER DEFAULT 0,
            reason TEXT DEFAULT '',
            created_at INTEGER
        );

CREATE TABLE IF NOT EXISTS chat_threads (
            user_id INTEGER PRIMARY KEY,
            last_body TEXT,
            last_at INTEGER DEFAULT 0,
            
            last_from_admin INTEGER DEFAULT 0,
            
            unread_user INTEGER DEFAULT 0,
            
            unread_admin INTEGER DEFAULT 0,
            
            
            
            
            
            
            
            chat_ver INTEGER DEFAULT 0
        );

CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            user_id INTEGER,
            from_admin INTEGER DEFAULT 0,
            body TEXT,
            
            media_file TEXT DEFAULT '',
            
            media_type TEXT DEFAULT '',
            media_ms INTEGER DEFAULT 0,
            seen INTEGER DEFAULT 0,
            created_at INTEGER
        );

CREATE TABLE IF NOT EXISTS tg_files (
            file_name TEXT PRIMARY KEY,
            msg_id INTEGER NOT NULL,
            file_key TEXT DEFAULT ''
        );

CREATE TABLE IF NOT EXISTS encode_jobs (
            anime_id INTEGER, season_id INTEGER, epizod_id INTEGER,
            origin TEXT NOT NULL,
            queued_at INTEGER NOT NULL,
            state TEXT DEFAULT 'queued',
            done TEXT DEFAULT '',
            runner TEXT DEFAULT '',
            lease_until INTEGER DEFAULT 0,
            attempts INTEGER DEFAULT 0,
            error TEXT DEFAULT '',
            PRIMARY KEY (anime_id, season_id, epizod_id)
        );

CREATE TABLE IF NOT EXISTS encode_q (
            anime_id INTEGER, season_id INTEGER, epizod_id INTEGER, quality TEXT,
            queued_at INTEGER NOT NULL,
            state TEXT DEFAULT 'queued',
            runner TEXT DEFAULT '',
            lease_until INTEGER DEFAULT 0,
            attempts INTEGER DEFAULT 0,
            error TEXT DEFAULT '',
            progress TEXT DEFAULT '',
            PRIMARY KEY (anime_id, season_id, epizod_id, quality)
        );

CREATE TABLE IF NOT EXISTS r2_files (
    file_name TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    anime_id INTEGER NOT NULL,
    season_id INTEGER NOT NULL,
    epizod_id INTEGER NOT NULL,
    queued_at INTEGER NOT NULL,
    quality TEXT NOT NULL,
    file_key TEXT NOT NULL,
    size INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'uploading',
    runner TEXT NOT NULL DEFAULT '',
    lease_until INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS vd_files (name TEXT PRIMARY KEY, size INTEGER NOT NULL, chunks INTEGER NOT NULL, vkey TEXT NOT NULL,           state TEXT NOT NULL, layout TEXT NOT NULL DEFAULT 'chunks', chunk_ids TEXT NOT NULL DEFAULT '', msg_ids TEXT NOT NULL DEFAULT '',           lease INTEGER NOT NULL DEFAULT 0, tries INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL);

CREATE TABLE IF NOT EXISTS vd_mig (name TEXT PRIMARY KEY, state TEXT NOT NULL, lease INTEGER NOT NULL DEFAULT 0,           tries INTEGER NOT NULL DEFAULT 0, err TEXT NOT NULL DEFAULT '', queued INTEGER NOT NULL);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_uname
            ON users_db(LOWER(username)) WHERE username <> '';

CREATE INDEX IF NOT EXISTS idx_users_created ON users_db(created_at);

CREATE INDEX IF NOT EXISTS idx_login_exp ON login_tokens(expires_at);

CREATE INDEX IF NOT EXISTS idx_sessions_user
            ON sessions_db(user_id, last_seen_at);

CREATE INDEX IF NOT EXISTS idx_sessions_seen
            ON sessions_db(last_seen_at);

CREATE INDEX IF NOT EXISTS idx_history_user
            ON watch_history_db(user_id, deleted_at, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pay_user
            ON payments_db(user_id, created_at);

CREATE INDEX IF NOT EXISTS idx_billing_user
            ON billing_log(user_id, created_at);

CREATE INDEX IF NOT EXISTS idx_comments_season
            ON comments_db(anime_id, season_id, parent_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_reports_new
            ON reports_db(created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_reports_once
            ON reports_db(kind, target_id, reporter_id);

CREATE INDEX IF NOT EXISTS idx_chat_threads_at
            ON chat_threads(last_at DESC);

CREATE INDEX IF NOT EXISTS idx_chat_msgs
            ON chat_messages(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_encode_q ON encode_jobs(state, queued_at);

CREATE INDEX IF NOT EXISTS vd_state ON vd_files(state, created);

CREATE INDEX IF NOT EXISTS vd_mig_state ON vd_mig(state, queued);
