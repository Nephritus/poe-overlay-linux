//! Routes from exile-leveling (MIT, vendored in `data/exile-leveling`), and the
//! Character's progress through them.
//!
//! Route lines are Steps such as `➞ {enter|1_1_town} #Lioneye's Watch`;
//! `{…}` fragments name game things by ID, `#…` ends the line as a comment,
//! `#sub` lines add Sub-steps to the previous Step and
//! `#ifdef`/`#ifndef`/`#endif` select variants. The Character's Area is
//! simulated through all acts from the start Area, as exile-leveling does:
//! `enter`, `waypoint` and `portal|use` move it; `logout` and `ascend` return
//! to the last town.

use std::collections::HashMap;

use serde::Deserialize;

const ACTS: [&str; 10] = [
    include_str!("../../data/exile-leveling/routes/act-1.txt"),
    include_str!("../../data/exile-leveling/routes/act-2.txt"),
    include_str!("../../data/exile-leveling/routes/act-3.txt"),
    include_str!("../../data/exile-leveling/routes/act-4.txt"),
    include_str!("../../data/exile-leveling/routes/act-5.txt"),
    include_str!("../../data/exile-leveling/routes/act-6.txt"),
    include_str!("../../data/exile-leveling/routes/act-7.txt"),
    include_str!("../../data/exile-leveling/routes/act-8.txt"),
    include_str!("../../data/exile-leveling/routes/act-9.txt"),
    include_str!("../../data/exile-leveling/routes/act-10.txt"),
];
const AREAS: &str = include_str!("../../data/exile-leveling/json/areas.json");
const QUESTS: &str = include_str!("../../data/exile-leveling/json/quests.json");

/// Attribution for the bundled Route data, with its license.
pub const CREDITS: &str = concat!(
    "Leveling Route data from exile-leveling by HeartofPhos\n",
    "https://github.com/HeartofPhos/exile-leveling\n",
    "Game names and text belong to Grinding Gear Games.\n\n",
    include_str!("../../data/exile-leveling/LICENSE"),
);

const START_AREA: &str = "1_1_1";
const START_TOWN: &str = "1_1_town";

/// Route variants, named after exile-leveling's build options; the defaults
/// are exile-leveling's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RouteOptions {
    /// Include side trips that pay off on a fresh league (e.g. early
    /// waypoints and vendor items).
    pub league_start: bool,
    pub bandit: Bandit,
    /// Include the Library in Act 3.
    pub library: bool,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self { league_start: true, bandit: Bandit::Alira, library: true }
    }
}

/// Which Act 2 bandit to help; `KillAll` helps none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bandit {
    KillAll,
    Oak,
    Kraityn,
    Alira,
}

impl RouteOptions {
    fn defines(&self) -> Vec<&'static str> {
        let mut defines = vec![match self.bandit {
            Bandit::KillAll => "BANDIT_KILL",
            Bandit::Oak => "BANDIT_OAK",
            Bandit::Kraityn => "BANDIT_KRAITYN",
            Bandit::Alira => "BANDIT_ALIRA",
        }];
        if self.league_start {
            defines.push("LEAGUE_START");
        }
        if self.library {
            defines.push("LIBRARY");
        }
        defines
    }
}

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
    areas: HashMap<String, Area>,
    problems: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Area {
    name: String,
    act: u32,
    is_town_area: bool,
    parent_town_area_id: Option<String>,
    crafting_recipes: Vec<String>,
}

#[derive(Deserialize)]
struct Quest {
    name: String,
}

impl Route {
    /// The bundled campaign Route, acts 1–10, in the `options` variant.
    pub fn campaign(options: &RouteOptions) -> Route {
        let areas: HashMap<String, Area> = serde_json::from_str(AREAS).expect("bundled areas.json");
        let quests: HashMap<String, Quest> = serde_json::from_str(QUESTS).expect("bundled quests.json");
        let mut parser = Parser {
            areas: &areas,
            quests: &quests,
            area: START_AREA.into(),
            town: START_TOWN.into(),
            portal: None,
            problems: Vec::new(),
            line: (0, 0),
        };
        let legs = parser.parse(&ACTS, &options.defines());
        let problems = parser.problems;
        Route { legs, areas, problems }
    }

    /// Route data the parser couldn't make sense of; empty for the bundled data.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }
}

struct Parser<'a> {
    areas: &'a HashMap<String, Area>,
    quests: &'a HashMap<String, Quest>,
    /// Where the Character is at this point of the Route.
    area: String,
    /// The town `logout` and `ascend` return to.
    town: String,
    /// The Area a placed portal leads back to.
    portal: Option<String>,
    problems: Vec<String>,
    /// Current act file and 1-based line, for problem reports.
    line: (usize, usize),
}

impl Parser<'_> {
    fn parse(&mut self, acts: &[&str], defines: &[&str]) -> Vec<Leg> {
        let mut legs: Vec<Leg> = Vec::new();
        for (act, source) in acts.iter().enumerate() {
            // One entry per open #ifdef/#ifndef: whether its lines apply.
            let mut conditions: Vec<bool> = Vec::new();
            for (n, line) in source.lines().enumerate() {
                self.line = (act + 1, n + 1);
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
                    if conditions.pop().is_none() {
                        self.problem("#endif without #ifdef");
                    }
                    continue;
                }
                if line.is_empty() || line.starts_with("#section") || conditions.contains(&false) {
                    continue;
                }
                if let Some(sub) = line.strip_prefix("#sub") {
                    let text = self.render(sub.trim());
                    match legs.last_mut().and_then(|leg| leg.steps.last_mut()) {
                        Some(step) => step.sub_steps.push(text),
                        None => self.problem("#sub before any Step"),
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
            if !conditions.is_empty() {
                self.problem("#ifdef without #endif");
            }
        }
        legs
    }

    fn problem(&mut self, what: &str) {
        let (act, line) = self.line;
        self.problems.push(format!("act-{act}.txt:{line}: {what}"));
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
            let Some(end) = rest[i..].find('}') else {
                self.problem("unclosed {");
                break;
            };
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
                self.push_area_name(arg, out);
            }
            "waypoint" if !arg.is_empty() => {
                self.move_to(arg);
                out.push_str("Waypoint ➞ ");
                self.push_area_name(arg, out);
            }
            "waypoint" | "waypoint_get" => out.push_str("Waypoint"),
            "logout" => {
                self.area.clone_from(&self.town);
                out.push_str("Logout ➞ ");
                self.push_area_name(&self.town.clone(), out);
            }
            "portal" => self.portal(arg, out),
            "ascend" => {
                self.area.clone_from(&self.town);
                out.push_str("Ascend (");
                out.push_str(arg);
                out.push_str(" Labyrinth)");
            }
            "crafting" => {
                let id = if arg.is_empty() { self.area.as_str() } else { arg };
                match self.areas.get(id) {
                    Some(area) => {
                        out.push_str("Crafting: ");
                        out.push_str(&area.crafting_recipes.join(", "));
                    }
                    None => self.problem(&format!("unknown area {id}")),
                }
            }
            "area" => self.push_area_name(arg, out),
            "quest" => match self.quests.get(arg) {
                Some(quest) => out.push_str(&quest.name),
                None => self.problem(&format!("unknown quest {arg}")),
            },
            "trial" => out.push_str("Trial of Ascendancy"),
            "dir" => out.push_str(direction(arg)),
            "kill" | "arena" | "quest_text" | "generic" => out.push_str(arg),
            _ => self.problem(&format!("unknown fragment {{{fragment}}}")),
        }
    }

    /// `{portal|set}` places a portal here; `{portal|use}` goes from town to
    /// the placed portal's Area, or from elsewhere to that Area's town.
    fn portal(&mut self, action: &str, out: &mut String) {
        let Some(here) = self.areas.get(&self.area) else {
            return self.problem("portal in an unknown area");
        };
        match action {
            "set" if !here.is_town_area => {
                self.portal = Some(self.area.clone());
                out.push_str("Portal");
            }
            "use" => {
                if !here.is_town_area && self.portal.as_ref() != Some(&self.area) {
                    self.portal = Some(self.area.clone());
                }
                let Some(portal) = self.portal.clone() else {
                    return self.problem("portal used before one was placed");
                };
                let destination = if portal == self.area {
                    here.parent_town_area_id.clone()
                } else if self.areas.get(&portal).and_then(|a| a.parent_town_area_id.as_ref()) == Some(&self.area) {
                    self.portal = None;
                    Some(portal)
                } else {
                    None
                };
                let Some(destination) = destination else {
                    return self.problem("portal used away from its town or area");
                };
                self.move_to(&destination);
                out.push_str("Portal ➞ ");
                self.push_area_name(&destination, out);
            }
            _ => self.problem(&format!("unknown portal action {action}")),
        }
    }

    fn move_to(&mut self, area_id: &str) {
        match self.areas.get(area_id) {
            Some(area) if area.is_town_area => area_id.clone_into(&mut self.town),
            Some(_) => {}
            None => self.problem(&format!("unknown area {area_id}")),
        }
        area_id.clone_into(&mut self.area);
    }

    fn push_area_name(&mut self, area_id: &str, out: &mut String) {
        match self.areas.get(area_id) {
            Some(area) => out.push_str(&area.name),
            None => {
                self.problem(&format!("unknown area {area_id}"));
                out.push_str(area_id);
            }
        }
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
        self.route.areas.get(id).map(|a| a.name.as_str())
    }

    /// Name of the Area the shown Steps are for.
    pub fn area_name(&self) -> &str {
        let id = &self.route.legs[self.current].area_id;
        self.route.areas.get(id).map_or(id, |a| &a.name)
    }

    /// Act of the Area the shown Steps are for.
    pub fn act(&self) -> u32 {
        self.route.areas.get(&self.route.legs[self.current].area_id).map_or(0, |a| a.act)
    }

    pub fn steps(&self) -> &[Step] {
        &self.route.legs[self.current].steps
    }
}
