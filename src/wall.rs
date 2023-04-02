use core::panic;

use std::env;
use std::ffi::OsStr;
use std::fs::{copy, create_dir};
use std::path::Path;
use std::process::Command;

use image::ImageError;
use image::{io::Reader as ImageReader, GenericImage};
use imageproc::{self, drawing};
use rusttype::{Font, Scale};

const FONT_SIZE_FACTOR: f32 = 0.45;

enum DeType {
    Gnome,
    Unknown,
    // Gnome3,
}

// trait Wallpaper {
//     fn set_wallpaper(&self, uri: String) -> Result<(), std::io::Error>;
//     fn get_wallpaper(&self) -> Result<String, std::io::Error>;
//     fn get_wallpaper_type(&self) -> Result<WallType, std::io::Error>;
//     fn toggle_wallpaper(&self) -> Result<(), std::io::Error> {
//         todo!{}
//     }
// }

enum WallType {
    Light,
    Dark,
}

fn get_backup_loc() -> String {
    let home_dir = env::var("HOME").unwrap();
    format!("{}/.todo-wallpaper/", home_dir)
}

fn get_de_type() -> DeType {
    match env::var("XDG_CURRENT_DESKTOP") {
        Err(_) => DeType::Unknown,
        Ok(de_name) => match de_name.as_str() {
            "GNOME" => DeType::Gnome,
            _ => DeType::Unknown,
        },
    }
}

// get command line from the desktop environment DeType
fn get_output_from_cmd<I, S>(cmd: &'static str, args: I) -> Result<String, std::io::Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Command::new(cmd).args(args).output()?;

    if !out.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "The command failed to execute",
        ));
    }

    match String::from_utf8(out.stdout) {
        Ok(output) => Ok(output),
        Err(e) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("The command output is not valid UTF-8: {}", e),
        )),
    }
}

/// Returns the paths of the original wallpaper and the writable wallpaper with "-todo-rs" suffix
/// # Arguments:
///    set_wallpaper_uri: the uri of the set wallpaper
fn get_old_and_new_wallpaper_uris(
    set_wallpaper_uri: &String,
) -> Result<(String, String), std::io::Error> {
    let backup_loc = get_backup_loc();
    let current_wallpaper_name = set_wallpaper_uri
        .rsplit_once("/")
        .unwrap()
        .1
        .replace("-todo-rs.", ".");
    let original_wall_bkup_loc = format!("{backup_loc}{current_wallpaper_name}");
    let todo_wall_loc = format!(
        "{backup_loc}{wall_file_name}-todo-rs.{wall_file_ext}",
        wall_file_name = current_wallpaper_name
            .replace("-todo-rs.", ".")
            .rsplit_once(".")
            .unwrap()
            .0,
        wall_file_ext = current_wallpaper_name.rsplit_once(".").unwrap().1
    );

    let dir_path = Path::new(&backup_loc);
    if !dir_path.exists() {
        // if the backup dir doesn't exist, and the wallpaper is a todo wallpaper
        // then the original wallpaper doesn't exist
        if current_wallpaper_name.contains("-todo-rs.") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "The original wallpaper doesn't exist",
            ));
        }

        // create the backup dir if it doesn't exist
        create_dir(dir_path)?;

        // create a copy of the wallpaper in the backup dir
        return match copy(&set_wallpaper_uri, &original_wall_bkup_loc) {
            Ok(_) => Ok((original_wall_bkup_loc, todo_wall_loc)),
            Err(e) => Err(e),
        };
    }

    // backup dir exists but the wallpaper doesn't exist in the backup dir
    if !Path::new(&original_wall_bkup_loc).exists() {
        return match copy(&set_wallpaper_uri, &original_wall_bkup_loc) {
            Ok(_) => Ok((original_wall_bkup_loc, todo_wall_loc)),
            Err(e) => Err(e),
        };
    }

    // return the todo wallpaper path to be overwritten along with the original wallpaper path
    Ok((original_wall_bkup_loc, todo_wall_loc))
}

fn de_dark_mode(de_type: DeType) -> WallType {
    match de_type {
        DeType::Gnome => {
            if let Ok(theme) = get_output_from_cmd(
                "gsettings",
                &["get", "org.gnome.desktop.interface", "color-scheme"],
            ) {
                if theme.contains("dark") {
                    return WallType::Dark;
                }
            }
            WallType::Light
        }
        DeType::Unknown => {
            println!("Info: Unknown desktop environment, defaulting to light theme");
            WallType::Light
        }
    }
}

pub fn set_wall(todos: &[String], dones: &[String]) {
    let set_wallpaper_uri = match get_de_type() {
        DeType::Gnome => {
            let dark_mode_on = de_dark_mode(DeType::Gnome);

            match get_output_from_cmd(
                "gsettings",
                &[
                    "get",
                    "org.gnome.desktop.background",
                    match dark_mode_on {
                        WallType::Dark => "picture-uri-dark",
                        WallType::Light => "picture-uri",
                    },
                ],
            ) {
                Ok(uri) => uri,
                Err(_) => return,
            }
        }
        DeType::Unknown => {
            eprintln!("Error: Unknown desktop environment encountered");
            return;
        }
    };

    let set_wallpaper_uri = set_wallpaper_uri[8..set_wallpaper_uri.len() - 2].to_string();
    let (original_wall_loc, todo_wall_loc) =
        match get_old_and_new_wallpaper_uris(&set_wallpaper_uri) {
            Ok((o, t)) => (o, t),
            Err(e) => {
                return eprintln!("Error: Could not get the old and new wallpapers URIs: {e}")
            }
        };

    // prepend todos and dones with ordered numbers
    let mut to_print = vec![];

    if todos.len() > 0 {
        to_print.push("TODOS:".to_string());
        to_print.push("".to_string());
        for (i, todo) in todos.iter().enumerate() {
            to_print.push(format!("{}. {}", i + 1, todo));
        }
    }
    if dones.len() > 0 {
        to_print.push("".to_string());
        to_print.push("DONES:".to_string());
        to_print.push("".to_string());
        for (i, done) in dones.iter().enumerate() {
            to_print.push(format!("{}. {}", i + 1, done));
        }
    }

    if todos.len() == 0 && dones.len() == 0 {
        return;
    }

    // if there are errors, then copy the original wallpaper into the todo wallpaper location
    if let Err(err) = write_wallpaper(&original_wall_loc, &todo_wall_loc, to_print.join("\n")) {
        if let Err(e) = copy(&original_wall_loc, &todo_wall_loc) {
            eprintln!(
                "Error: Could not copy the original wallpaper to the todo wallpaper location: {e}"
            );
        }
        eprintln!("Error: Could not write the todo wallpaper: {err}");
    }

    // FIXME: this is specific to GNOME
    let dark_mode_on = de_dark_mode(DeType::Gnome);
    let todo_wall_loc = format!("file://{todo_wall_loc}");

    // update the gsettings to set the new wallpaper
    if let Err(err) = get_output_from_cmd(
        "gsettings",
        &[
            "set",
            "org.gnome.desktop.background",
            match dark_mode_on {
                WallType::Dark => "picture-uri-dark",
                WallType::Light => "picture-uri",
            },
            todo_wall_loc.as_str(),
        ],
    ) {
        eprintln!("Error: Could not set the todo wallpaper: {err}");
    }
}

fn write_wallpaper(
    input_image_path: &str,
    output_image_path: &str,
    text: String,
) -> Result<(), ImageError> {
    let mut img = ImageReader::open(input_image_path)?.decode()?;
    let dark_mode_on = de_dark_mode(DeType::Gnome);

    let (img_width, img_height) = (img.width(), img.height());

    if img_width < 200 || img_height < 200 {
        panic!("Image is too small");
    }

    // 1. divide the image into 2 vertical halves if width < 500 px
    // else divide into 3 vertical parts
    let (start_x, start_y) = (
        (if img_width < 500 {
            img_width / 2
        } else {
            2 * img_width / 3
        }),
        0u32,
    );

    // 2. scale the font according to the image dimensions (with min and max - clamp)
    // FIXME: make use of screen dimensions instead of image dimensions
    let font_size = (img_height as f32)
        .powf(FONT_SIZE_FACTOR)
        .clamp(8f32, 128f32);
    let font_size = Scale::uniform(font_size);
    let font = Font::try_from_bytes(include_bytes!("../fonts/Ubuntu-M.ttf") as &[u8]).unwrap();

    let text_color = match dark_mode_on {
        WallType::Dark => image::Rgba([255u8, 255u8, 255u8, 255u8]),
        WallType::Light => image::Rgba([0u8, 0u8, 0u8, 255u8]),
    };

    // 3. wrap the text according to the space available
    // NOTE: font width calculation is slightly off, so 0.02 is a hack instead of 0.04
    let wrapped_string = wrap_string(
        text,
        // (img_width - start_x - (0.02 * img_width as f32) as u32) as usize,
        (img_width - start_x as u32) as usize,
        &font,
        &font_size,
    );

    // 4. blur the right portion of the image starting from start_x
    let blurred_img = img
        .crop(
            start_x as u32,
            0u32,
            (img_width - start_x) as u32,
            img_height as u32,
        )
        .brighten(
            30i32
                * match dark_mode_on {
                    WallType::Dark => -1,
                    WallType::Light => 1,
                },
        )
        .blur(15f32);
    img.copy_from(&blurred_img, start_x as u32, 0)?;

    // 5. add text to the image leaving a margin of 2% of the image width and height
    wrapped_string.iter().enumerate().for_each(|(i, line)| {
        drawing::draw_text_mut(
            &mut img,
            text_color,
            start_x as i32 + (0.02 * img_width as f32) as i32,
            start_y as i32
                + ((1.15 * i as f32 * font_size.y) as i32)
                + (0.1 * img_height as f32) as i32,
            font_size,
            &font,
            line,
        );
    });

    // 6. save the image
    img.save(output_image_path)?;
    Ok(())
}

/// Wraps a string to a given width.
/// - `text` - The string to wrap.
/// - `bounding_box_width` - The width to wrap the string to.
/// - `font` - The font to use.
/// - `scale` - The scale to use.
///
/// Returns a vector of strings, each of which is guaranteed to be no wider than the given width.
fn wrap_string(text: String, bounding_box_width: usize, font: &Font, scale: &Scale) -> Vec<String> {
    // since space does not produce a bounding box, we need to calculate the width of a space
    // by using a period as a reference
    let space_width = font
        .layout(" .", *scale, rusttype::point(0.0, 0.0))
        .map(|g| {
            if let Some(val) = g.pixel_bounding_box() {
                return val.min.x as f32;
            }
            0f32
        })
        .sum::<f32>();

    let mut output: Vec<String> = vec![];
    let mut current_line: Vec<_> = vec![];
    let mut current_line_width = 0f32;

    // This is the best way to handle this that I can think of.
    // LayoutIter does not provide the character that it is currently on, so we have to
    // manually split the string into lines and words.
    // Moreover, blank spaces have no bounding box, so we are out of luck there as well
    // in terms of character counting.
    for line in text.split('\n') {
        for word in line.split(' ') {
            let word_width = font
                .layout(word, *scale, rusttype::point(0.0, 0.0))
                .map(|g| match g.pixel_bounding_box() {
                    Some(val) => val.max.x as f32 - val.min.x as f32 + space_width,
                    None => 0f32,
                })
                .sum::<f32>();

            if word_width + current_line_width >= bounding_box_width as f32 {
                output.push(current_line.join(" "));
                current_line = vec![];
                current_line_width = 0f32;
            }

            current_line.push(word);
            current_line_width += word_width;
        }

        output.push(current_line.join(" "));
        current_line = vec![];
        current_line_width = 0f32;
    }

    output
}

/*
 * 1. get the wallpaper uri from the desktop environment
 * 2. if the wallpaper name does not contain "todo" suffix
 *    2.1 make two copies of the wallpaper in the backup location
 *    2.2 write the todo on the copy with "todo" suffix
 * 3. if the wallpaper name contains "todo" suffix
 *    3.1 delete the wallpaper with "todo" suffix
 *    3.2 copy the wallpaper without "todo" suffix with "todo" suffix
 *    3.3 write the todo on the copy with "todo" suffix
 * 5. set the wallpaper
 */

// TODO: clean the directory of any old wallpapers
// TODO: do not render the todo wallpaper if the todos remain the same
// TODO: indent the lines that have been wrapped
