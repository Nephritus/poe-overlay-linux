//! Routes from exile-leveling (MIT, vendored in `data/exile-leveling`), and the
//! Character's progress through them.
//!
//! Route lines are Steps such as `➞ {enter|1_1_town} #Lioneye's Watch`;
//! `{…}` fragments name game things by ID, `#…` ends the line as a comment,
//! `#sub` lines annotate the previous Step and `#ifdef`/`#ifndef`/`#endif`
//! select variants. The Character's Area is simulated from the start Area:
//! `enter` and `waypoint` move it, `logout` returns to the last town.

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
            let area_before = self.area.clone();
            let step = Step { text: self.render(line), sub_steps: Vec::new() };
            match legs.last_mut() {
                Some(leg) if leg.area_id == area_before => leg.steps.push(step),
                _ => legs.push(Leg { area_id: area_before, steps: vec![step] }),
            }
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
            let text = self.fragment(&rest[i + 1..i + end]);
            // `3x{quest_text|Glyph}` reads "3x Glyph" as plain text.
            if out.ends_with(char::is_alphanumeric) && text.starts_with(char::is_alphanumeric) {
                out.push(' ');
            }
            out.push_str(&text);
            rest = &rest[i + end + 1..];
        }
        out.push_str(rest);
        out.trim().to_owned()
    }

    fn fragment(&mut self, fragment: &str) -> String {
        let mut parts = fragment.split('|');
        let kind = parts.next().unwrap_or_default();
        let arg = parts.next().unwrap_or_default();
        match kind {
            "enter" => {
                self.move_to(arg);
                self.area_name(arg)
            }
            "waypoint" if !arg.is_empty() => {
                self.move_to(arg);
                format!("Waypoint ➞ {}", self.area_name(arg))
            }
            "waypoint" | "waypoint_get" => "Waypoint".into(),
            "logout" => {
                let town = self.town.clone();
                self.move_to(&town);
                format!("Logout ➞ {}", self.area_name(&town))
            }
            "area" => self.area_name(arg),
            "quest" => self.quests.get(arg).map_or_else(|| arg.to_owned(), |q| q.name.clone()),
            "trial" => "Trial of Ascendancy".into(),
            "dir" => direction(arg).into(),
            // kill, arena, quest_text, generic: the argument is the text.
            _ => arg.to_owned(),
        }
    }

    fn move_to(&mut self, area_id: &str) {
        if self.areas.get(area_id).is_some_and(|a| a.is_town_area) {
            self.town = area_id.to_owned();
        }
        self.area = area_id.to_owned();
    }

    fn area_name(&self, area_id: &str) -> String {
        self.areas.get(area_id).map_or_else(|| area_id.to_owned(), |a| a.name.clone())
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
}

impl RouteProgress {
    pub fn new(route: Route) -> Self {
        Self { route, current: 0 }
    }

    /// Moves to the Steps for the Area the Character just entered: the
    /// nearest Leg for that Area, ahead on a tie. Stepping back to the
    /// previous Area or a town trip mid-Area finds the Leg just behind; an
    /// Area the Route doesn't visit (e.g. a hideout) changes nothing.
    /// Returns whether the Steps changed.
    pub fn enter_area(&mut self, area_id: &str) -> bool {
        let legs = &self.route.legs;
        if legs[self.current].area_id == area_id {
            return false;
        }
        let ahead = (self.current + 1..legs.len()).find(|&i| legs[i].area_id == area_id);
        let behind = (0..self.current).rev().find(|&i| legs[i].area_id == area_id);
        let nearest = match (ahead, behind) {
            (Some(a), Some(b)) if self.current - b < a - self.current => Some(b),
            (Some(a), _) => Some(a),
            (None, b) => b,
        };
        match nearest {
            Some(i) => {
                self.current = i;
                true
            }
            None => false,
        }
    }

    pub fn area_name(&self) -> &str {
        let id = &self.route.legs[self.current].area_id;
        self.route.area_names.get(id).map_or(id, String::as_str)
    }

    pub fn steps(&self) -> &[Step] {
        &self.route.legs[self.current].steps
    }
}
