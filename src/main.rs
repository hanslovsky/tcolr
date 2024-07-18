use ansi_term::Colour;
use ansi_term::Colour::RGB;
use bytes::Bytes;
use clap::Parser;
use image::DynamicImage;
use image::ImageBuffer;
use image::ImageError;
use image::{Rgb, Rgba};
use image::Pixel;
use image::io::Reader;
use reqwest;
use std::error::Error;
use std::fmt;
use std::io::Cursor;
use std::ops::Deref;
use std::option::Option;
use std::str;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Debug)]
enum ImageFromUriError {
    NoSchemeSpecified(String),
    UnsupportedScheme((String, String)),
    ImageError(ImageError),
    Generic(Box<dyn Error>)
}

impl fmt::Display for ImageFromUriError {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageFromUriError::ImageError(e) => write!(f, "{}", e),
            ImageFromUriError::UnsupportedScheme(e) => write!(f, "({}, {})", e.0, e.1),
            ImageFromUriError::Generic(b) => write!(f, "{}", b),
            ImageFromUriError::NoSchemeSpecified(u) => write!(f, "{}", u)
        }
    }
}

impl From<String> for ImageFromUriError {
    fn from(uri: String) -> Self {
        return ImageFromUriError::NoSchemeSpecified(uri);
    }
}

impl From<(String, String)> for ImageFromUriError {
    fn from(schema_url: (String, String)) -> Self {
        ImageFromUriError::UnsupportedScheme(schema_url)
    }
}

fn get_image(image_url: &String) -> Result<DynamicImage, ImageFromUriError> {
    let image = match image_url {
        u if !u.contains("://") => get_image_from_file(u),
        u if u.starts_with("file://") => get_image(&u.strip_prefix("file://").unwrap_or_default().to_owned()),
        u if u.starts_with("http://") || u.starts_with("https://") => get_image_from_https(u),
        u => Err(ImageFromUriError::from((
            u.split_once("://").map(|t| t.0).unwrap_or_default().to_owned(),
            u.clone())))
    };
    return image;
}


fn get_image_from_bytes(bytes: Bytes) -> Result<DynamicImage, ImageFromUriError> {
    let c = Cursor::new(bytes);
    let reader = Reader::new(c).with_guessed_format();
    match reader {
        Ok(r) => match r.decode() {
            Ok(img) => Ok(img),
            Err(e) => Err(ImageFromUriError::Generic(Box::new(e)))
        },
        Err(e) => Err(ImageFromUriError::Generic(Box::new(e)))
    }
}

fn get_image_from_https(url: &String) -> Result<DynamicImage, ImageFromUriError> {
    let response = reqwest::blocking::get(url);
    let image = match response {
        Ok(r) => match r.bytes() {
            Ok(b) => get_image_from_bytes(b),
            Err(e) => Err(ImageFromUriError::Generic(Box::new(e)))
        },
        Err(e) => Err(ImageFromUriError::Generic(Box::new(e)))
    };
    return image;
}

fn get_image_from_file(path: &String) -> Result<DynamicImage, ImageFromUriError> {
    let image = Reader::open(path);
    return match image {
        Ok(i) => i.decode().map_err(|e| ImageFromUriError::ImageError(e)),
        Err(e) => Err(ImageFromUriError::Generic(Box::new(e)))
    }
}

pub struct RGBSum {
    r: u64,
    g: u64,
    b: u64,
}

impl RGBSum {
    fn zero() -> RGBSum {
        RGBSum { r: 0, g: 0, b: 0 }
    }

    fn set_zero(&mut self) -> &RGBSum {
        self.r = 0;
        self.g = 0;
        self.b = 0;
        return self;
    }

    fn add(&mut self, other: &RGBSum) {
        self.r += other.r;
        self.g += other.g;
        self.b += other.b;
    }

    fn div(&mut self, n: u64) {
        self.r /= n;
        self.g /= n;
        self.b /= n;
    }
}

trait Div<T> {
   fn div_inplace(&mut self, divisor: T);
}

trait IsSame {
    fn is_same(&self, color: &Colour) -> bool;
}

impl Div<u64> for RGBSum {
    fn div_inplace(&mut self, divisor: u64) {
        self.div(divisor);
    }
}

impl IsSame for RGBSum {
    fn is_same(&self, color: &Colour) -> bool {
        return match color {
            // TODO can we avoid r.clone().into() here?
            Colour::RGB(r, g, b ) => self.r == r.clone().into() && self.g == g.clone().into() && self.b == b.clone().into(),
            _ => false
        }
    }
}

trait ToColour {
    fn to_colour(&self) -> Colour;
}

impl ToColour for RGBSum {
    fn to_colour(&self) -> Colour {
        return Colour::RGB(self.r as u8, self.g as u8, self.b as u8);
    }
}

trait Aggregator<P>: Div<u64> {
    fn aggregate(&mut self, p: &P);
}

impl <U: Into<u64> + Copy> Aggregator<Rgb<U>> for RGBSum {
    fn aggregate(&mut self, p: &Rgb<U>) {
        self.r += p[0].into();
        self.g += p[1].into();
        self.b += p[2].into();
    }
}

impl <U: Into<u64> + Copy> Aggregator<Rgba<U>> for RGBSum {
    fn aggregate(&mut self, p: &Rgba<U>) {
        self.r += p[0].into();
        self.g += p[1].into();
        self.b += p[2].into();
    }
}

impl Clone for RGBSum {
    fn clone(&self) -> Self {
        return RGBSum {
            r: self.r,
            g: self.g,
            b: self.b
        }
    }
}

fn sum_chunks_inplace<P: Pixel, Agg: Aggregator<P>, C: Deref<Target = [P::Subpixel]>>(
    image: &ImageBuffer<P, C>,
    chunk_width: u32,
    row: u32,
    target: &mut [Agg]
) {
    let n = image.width() / chunk_width;
    for idx in 0 .. n {
        let start = idx * chunk_width;
        let stop = start + chunk_width;
        for x in start .. stop {
            let rgb = image.get_pixel(x, row);
            target[idx as usize].aggregate(rgb);
        }
    }
}

struct RgbCount {
    rgb_sum: RGBSum,
    count: usize
}


impl RgbCount {
    fn is_same_rgb(&self, other: &RGBSum) -> bool {
        return self.rgb_sum.r == other.r && self.rgb_sum.g == other.g && self.rgb_sum.b == other.b
    }

    fn incr(&mut self) {
        self.count = self.count + 1
    }

    fn is_valid(&self) -> bool {
        self.rgb_sum.r < 256 && self.rgb_sum.g < 256 && self.rgb_sum.b < 256
    }

    fn set_rgb(&mut self, rgb: RGBSum) {
        self.rgb_sum = rgb;
        self.count = 1;
    }

    fn invalid() -> RgbCount {
        RgbCount {
            rgb_sum: RGBSum { r: 256, g: 0, b: 0 },
            count: 1
        }
    }
}


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {

    #[arg(short, long, default_value_t = String::from("/home/zottel/Pictures/atze.jpg"))]
    image_url: String,

    #[arg(short, long, default_value_t = 20)]
    x_chunks: usize,

    #[arg(short, long, default_value_t = 40)]
    y_chunks: usize,

}

fn get_time() -> Duration {
    return SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
}


fn main() {

    let t0 = get_time();
    let args = Args::parse();
    let t1 = get_time();

    let chunks_x = args.x_chunks;
    let chunks_y = args.y_chunks;

    let image = match get_image(&args.image_url) {
        Ok(i) => match i {
            DynamicImage::ImageRgb8(buf) => parse_image_and_print(&buf, chunks_x, chunks_y, RGBSum::zero()),
            _ => panic!("Unsupported pixel type:")
        }
        Err(error) => panic!("Unable to open image for uri {}: {:?}", args.image_url, error)
    };
}

fn parse_image_and_print<P: Pixel, Agg: Aggregator<P> + Clone + IsSame + ToColour, C: Deref<Target = [P::Subpixel]>>(
    buf: &ImageBuffer<P, C>,
    chunks_x: usize,
    chunks_y: usize,
    zero_agg: Agg
) {
    let n_x = buf.width() as usize / chunks_x;
    let n_y = buf.height() as usize / chunks_y;
    let n = chunks_x as u64 * chunks_y as u64;

    let mut rgbs = vec![zero_agg.clone(); n_x * n_y];

    for y_chunk in 0 .. n_y {
        let start = y_chunk * chunks_y;
        let stop = start + chunks_y;

        let mut slice = &mut rgbs[y_chunk * n_x .. y_chunk * n_x + n_x];
        for y in start .. stop {
            sum_chunks_inplace(&buf, chunks_x as u32, y as u32, slice);
        }
        for rgb in slice {
            rgb.div_inplace(n as u64);
        }
    }
    
    for y_chunk in 0 .. n_y {
        let slice = &rgbs[y_chunk * n_x .. y_chunk * n_x + n_x];
        let mut prev: Colour = Colour::Black;
        let mut counter: usize = 0;
        for rgb in slice {
            match counter {
                0 => {
                    prev = rgb.to_colour();
                    counter += 1;
                },
                _ => {
                    if rgb.is_same(&prev) {
                        counter += 1;
                    } else {
                        print!("{}", prev.paint("$".repeat(counter)));
                        counter = 0;
                    }
                }
            }
        }
        print!("{}", prev.paint("$".repeat(counter)));
        print!("\n");
    }
}
