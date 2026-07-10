# Daily Runs and Streaks

## Story
As a player, I want each workday to be a "daily run" with a goal, and a streak flame that grows for every consecutive daily goal I hit, so that showing up and keeping the queue healthy becomes a habit I don't want to break.

## Requirements

### Requirement: Daily run and streak tracking
The system SHALL define a per-player daily goal (e.g. hurdles cleared or XP earned), SHALL increment the player's streak when the goal is met, SHALL reset the streak when a day's goal is missed while remembering the personal best, and SHALL display the streak flame on the player's profile and home screen.

#### Scenario: Daily goal met
- **WHEN** a player reaches the daily goal
- **THEN** the run is marked complete, the streak increments, and a streak animation plays

#### Scenario: Streak broken
- **WHEN** a day ends below the goal
- **THEN** the streak resets to zero and the previous best is kept as a record

#### Scenario: Streak insurance
- **WHEN** a player misses one day but had a streak of 7 or more
- **THEN** one "streak shield" (earned, not bought) may automatically preserve the streak
