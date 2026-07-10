CREATE TABLE daily_runs (
    player_id INTEGER NOT NULL,
    run_date TEXT NOT NULL,
    progress_xp INTEGER NOT NULL DEFAULT 0,
    goal_met INTEGER NOT NULL DEFAULT 0 CHECK (goal_met IN (0, 1)),
    PRIMARY KEY (player_id, run_date)
);

CREATE TABLE player_streaks (
    player_id INTEGER PRIMARY KEY,
    current_streak INTEGER NOT NULL DEFAULT 0,
    best_streak INTEGER NOT NULL DEFAULT 0,
    has_shield INTEGER NOT NULL DEFAULT 0 CHECK (has_shield IN (0, 1)),
    last_settled_date TEXT
);
