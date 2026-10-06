//! A pocket's toolpath as a G-code program: millimetres, absolute
//! coordinates, the spindle on for the cut. Every run starts from the
//! clearance height over the stock, goes down at the plunge rate, cuts at
//! the feed rate and rises back before the next.

use crate::Pocket;
use crate::path::Toolpath;

/// The program for `path`, the toolpath of the operation `name`.
pub fn write(name: &str, pocket: &Pocket, path: &Toolpath) -> String {
    // Parentheses end a comment early.
    let plain = |text: &str| text.replace(['(', ')'], "");
    let safe = pocket.top + pocket.clearance;
    let floor = path.levels.last().copied().unwrap_or(pocket.top);
    let mut g = String::new();
    let mut line = |text: String| {
        g.push_str(&text);
        g.push('\n');
    };
    line("%".into());
    line(format!("(printCAD example CAM: {})", plain(name)));
    line(format!(
        "(tool: {}, diameter {:.3} mm)",
        plain(&pocket.tool),
        pocket.tool_diameter
    ));
    line(format!(
        "(stock top Z{:.3}, floor Z{floor:.3}, {} passes)",
        pocket.top,
        path.levels.len()
    ));
    line("G21 (millimetres)".into());
    line("G90 (absolute)".into());
    line("G17 (XY plane)".into());
    line(format!("G0 Z{safe:.3}"));
    line(format!("S{:.0} M3", pocket.spindle));
    for (pass, z) in path.levels.iter().enumerate() {
        line(format!(
            "(pass {} of {} at Z{z:.3})",
            pass + 1,
            path.levels.len()
        ));
        for run in &path.runs {
            let Some((first, rest)) = run.split_first() else {
                continue;
            };
            line(format!("G0 X{:.3} Y{:.3}", first[0], first[1]));
            line(format!("G1 Z{z:.3} F{:.0}", pocket.plunge));
            let mut moves = rest.iter();
            if let Some(p) = moves.next() {
                line(format!("G1 X{:.3} Y{:.3} F{:.0}", p[0], p[1], pocket.feed));
            }
            for p in moves {
                line(format!("G1 X{:.3} Y{:.3}", p[0], p[1]));
            }
            line(format!("G0 Z{safe:.3}"));
        }
    }
    line("M5".into());
    line("M30".into());
    line("%".into());
    g
}
