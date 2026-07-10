# League Leaderboard

## Story
As a team member, I want a weekly league table ranking runners by XP, streaks, and speed, so that clearing the backlog together has the friendly competitive pull of a mobile game league.

## Requirements

### Requirement: League table
The system SHALL display a leaderboard ranking players by XP for a selectable period (today, this week, all time), including streaks, badges, and average response time, and SHALL update live as hurdles are cleared.

#### Scenario: Live rank change
- **WHEN** a player clears a hurdle and overtakes a teammate's XP
- **THEN** the leaderboard animates the rank swap without manual refresh

#### Scenario: Period switch
- **WHEN** a user switches the league from "this week" to "today"
- **THEN** rankings recompute using only today's XP

#### Scenario: Team over individual
- **WHEN** the league is viewed
- **THEN** a team total (all runners combined vs. the incoming volume) is shown above individual ranks, so the framing stays collaborative
