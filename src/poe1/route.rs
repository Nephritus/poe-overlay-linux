//! Routes from exile-leveling (MIT, vendored in `data/exile-leveling`), and the
//! Character's progress through them.
//!
//! Route lines are Steps such as `➞ {enter|1_1_town} #Lioneye's Watch`;
//! `{…}` fragments name game things by ID, `#…` ends the line as a comment,
//! `#sub` lines add Sub-steps to the previous Step and
//! `#ifdef`/`#ifndef`/`#endif` select variants. The Character's Area is
//! simulated from the start Area: `enter` and `waypoint` move it, `logout`
//! returns to the last town. Only the fragments Act 1 uses are understood;
//! others render their argument as text.

use std::collections::HashMap;

use serde::Deserialize;

const ACT_1: &str = include_str!("../../data/exile-leveling/routes/act-1.txt");
const AREAS: &str = include_str!("../../data/exile-leveling/json/areas.json");
const QUESTS: &str = include_str!("../../data/exile-leveling/json/quests.json");

const START_AREA: &str = "1_1_1";
const START_TOWN: &str = "1_1_town";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub text: String,
    pub sub_steps: Vec<String>,
}

/// The Steps performed in one Area, ending with the Step that leaves it.
#[derive(Debug)]
struct Leg {
    area_id: String,
    steps: Vec<Step>,
}

#[derive(Debug)]
pub struct Route {
    legs: Vec<Leg>,
    area_names: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Area {
    name: String,
    is_town_area: bool,
}

#[derive(Deserialize)]
struct Quest {
    name: String,
}

impl Route {
    /// The bundled Act 1 Route; `league_start` includes its league-start-only Steps.
    pub fn act1(league_start: bool) -> Route {
        let areas: HashMap<String, Area> = serde_json::from_str(AREAS).expect("bundled areas.json");
        let quests: HashMap<String, Quest> = serde_json::from_str(QUESTS).expect("bundled quests.json");
        let mut defines = Vec::new();
        if league_start {
            defines.push("LEAGUE_START");
        }
        let legs = Parser { areas: &areas, quests: &quests, area: START_AREA.into(), town: START_TOWN.into() }
            .parse(ACT_1, &defines);
        let area_names = areas.into_iter().map(|(id, area)| (id, area.name)).collect();
        Route { legs, area_names }
    }
}

struct Parser<'a> {
    areas: &'a HashMap<String, Area>,
    quests: &'a HashMap<String, Quest>,
    /// Where the Character is at this point of the Route.
    area: String,
    /// The town `logout` returns to.
    town: String,
}

impl Parser<'_> {
    fn parse(mut self, source: &str, defines: &[&str]) -> Vec<Leg> {
        let mut legs: Vec<Leg> = Vec::new();
        // One entry per open #ifdef/#ifndef: whether its lines apply.
        let mut conditions: Vec<bool> = Vec::new();
        for line in source.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix("#ifdef ") {
                conditions.push(defines.contains(&name.trim()));
                continue;
            }
            if let Some(name) = line.strip_prefix("#ifndef ") {
                conditions.push(!defines.contains(&name.trim()));
                continue;
            }
            if line.starts_with("#endif") {
                conditions.pop();
                continue;
            }
            if line.is_empty() || line.starts_with("#section") || conditions.contains(&false) {
                continue;
            }
            if let Some(sub) = line.strip_prefix("#sub") {
                let text = self.render(sub.trim());
                if let Some(step) = legs.last_mut().and_then(|leg| leg.steps.last_mut()) {
                    step.sub_steps.push(text);
                }
                continue;
            }
            // A Step belongs to the Area the Character is in before doing it.
            let new_leg = legs.last().is_none_or(|leg| leg.area_id != self.area);
            if new_leg {
                legs.push(Leg { area_id: self.area.clone(), steps: Vec::new() });
            }
            let step = Step { text: self.render(line), sub_steps: Vec::new() };
            legs.last_mut().expect("pushed above").steps.push(step);
        }
        legs
    }

    /// Renders a line's text and `{…}` fragments, dropping any `#` comment,
    /// and moves the simulated Character as the fragments say.
    fn render(&mut self, line: &str) -> String {
        let mut out = String::new();
        let mut rest = line;
        while let Some(i) = rest.find(['{', '#']) {
            out.push_str(&rest[..i]);
            if rest[i..].starts_with('#') {
                rest = "";
                break;
            }
            let Some(end) = rest[i..].find('}') else { break };
            let start = out.len();
            self.fragment(&rest[i + 1..i + end], &mut out);
            // `3x{quest_text|Glyph}` reads "3x Glyph" as plain text.
            if out[..start].ends_with(char::is_alphanumeric) && out[start..].starts_with(char::is_alphanumeric) {
                out.insert(start, ' ');
            }
            rest = &rest[i + end + 1..];
        }
        out.push_str(rest);
        out.truncate(out.trim_end().len());
        out
    }

    fn fragment(&mut self, fragment: &str, out: &mut String) {
        let mut parts = fragment.split('|');
        let kind = parts.next().unwrap_or_default();
        let arg = parts.next().unwrap_or_default();
        match kind {
            "enter" => {
                self.move_to(arg);
                out.push_str(self.area_name(arg));
            }
            "waypoint" if !arg.is_empty() => {
                self.move_to(arg);
                out.push_str("Waypoint ➞ ");
                out.push_str(self.area_name(arg));
            }
            "waypoint" | "waypoint_get" => out.push_str("Waypoint"),
            "logout" => {
                self.area.clone_from(&self.town);
                out.push_str("Logout ➞ ");
                out.push_str(self.area_name(&self.town));
            }
            "area" => out.push_str(self.area_name(arg)),
            "quest" => out.push_str(self.quests.get(arg).map_or(arg, |q| &q.name)),
            "trial" => out.push_str("Trial of Ascendancy"),
            "dir" => out.push_str(direction(arg)),
            // kill, arena, quest_text, generic: the argument is the text.
            _ => out.push_str(arg),
        }
    }

    fn move_to(&mut self, area_id: &str) {
        if self.areas.get(area_id).is_some_and(|a| a.is_town_area) {
            area_id.clone_into(&mut self.town);
        }
        area_id.clone_into(&mut self.area);
    }

    fn area_name<'a>(&'a self, area_id: &'a str) -> &'a str {
        self.areas.get(area_id).map_or(area_id, |a| &a.name)
    }
}

/// `{dir|N}`: compass degrees in steps of 45, 0 = up.
fn direction(degrees: &str) -> &'static str {
    const ARROWS: [&str; 8] = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];
    let degrees: i64 = degrees.parse().unwrap_or(0);
    ARROWS[(degrees.rem_euclid(360) / 45) as usize]
}

/// Where the Character is in a Route.
#[derive(Debug)]
pub struct RouteProgress {
    route: Route,
    current: usize,
    /// The Area the Character is in when the Route doesn't visit it.
    off_route_area: Option<String>,
    /// The Leg left by a jump that wasn't a step forward (a town trip,
    /// stepping back); entering its Area again returns there.
    detour_from: Option<usize>,
}

impl RouteProgress {
    pub fn new(route: Route) -> Self {
        Self { route, current: 0, off_route_area: None, detour_from: None }
    }

    /// Moves to the Steps for the Area the Character just entered.
    ///
    /// Returning from a detour goes back to the Leg it left. Otherwise the
    /// nearest Leg for that Area wins, ahead on a tie; any jump other than a
    /// step forward starts a detour. An Area the Route doesn't visit (e.g. a
    /// hideout) keeps the Steps and marks the Character off the Route.
    /// Returns whether anything shown changed.
    pub fn enter_area(&mut self, area_id: &str) -> bool {
        let legs = &self.route.legs;
        let was_off_route = self.off_route_area.take().is_some();
        if legs[self.current].area_id == area_id {
            return was_off_route;
        }
        if let Some(left) = self.detour_from.filter(|&left| legs[left].area_id == area_id) {
            self.current = left;
            self.detour_from = None;
            return true;
        }
        let ahead = (self.current + 1..legs.len()).find(|&i| legs[i].area_id == area_id);
        let behind = (0..self.current).rev().find(|&i| legs[i].area_id == area_id);
        let nearest = match (ahead, behind) {
            (Some(a), Some(b)) if self.current - b < a - self.current => Some(b),
            (Some(a), _) => Some(a),
            (None, b) => b,
        };
        let Some(next) = nearest else {
            self.off_route_area = Some(area_id.to_owned());
            return true;
        };
        if next == self.current + 1 {
            self.detour_from = None;
        } else if self.detour_from.is_none() {
            self.detour_from = Some(self.current);
        }
        self.current = next;
        true
    }

    /// Whether the Character is in an Area the Route doesn't visit; the
    /// Steps are then the last ones it was on.
    pub fn is_off_route(&self) -> bool {
        self.off_route_area.is_some()
    }

    /// Name of the Area the Character is in, if the game data knows it
    /// (hideouts aren't in it).
    pub fn character_area_name(&self) -> Option<&str> {
        let id = self.off_route_area.as_ref().unwrap_or(&self.route.legs[self.current].area_id);
        self.route.area_names.get(id).map(String::as_str)
    }

    pub fn area_name(&self) -> &str {
        let id = &self.route.legs[self.current].area_id;
        self.route.area_names.get(id).map_or(id, String::as_str)
    }

    pub fn steps(&self) -> &[Step] {
        &self.route.legs[self.current].steps
    }
}
