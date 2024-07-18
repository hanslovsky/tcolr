use ansi_term::Colour::RGB;
use bytes::Bytes;
use clap::Parser;
use image::DynamicImage;
use image::ImageBuffer;
use image::ImageError;
use image::{Rgb, Rgba};
use image::Pixel;
use image::Primitive;
use image::io::Reader;
use reqwest;
use std::error::Error;
use std::fmt;
use std::io::Cursor;
use std::ops::Deref;
use std::str;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

trait TColRError: Error + fmt::Display {}

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

pub trait Aggregator<P> {
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

// impl <U: Into<u64> + Copy> AddRGB<Rgb<U>> for RGBSum {
//     fn add_rgb(&mut self, rgb: &Rgb<U>) {
//         self.r += rgb[0].into();
//         self.g += rgb[1].into();
//         self.b += rgb[2].into();
//     }
// }

// impl <U: Into<u64> + Copy> AddRGB<Rgba<U>> for RGBSum {
//     fn add_rgb(&mut self, rgb: &Rgba<U>) {
//         self.r += rgb[0].into();
//         self.g += rgb[1].into();
//         self.b += rgb[2].into();
//     }
// }

impl Clone for RGBSum {
    fn clone(&self) -> Self {
        return RGBSum {
            r: self.r,
            g: self.g,
            b: self.b
        }
    }
}


// fn sum_chunks<P: Pixel, C: Deref<Target = [P::Subpixel]>>(
//     image: &ImageBuffer<P, C>,
//     chunk_width: u32,
//     row: u32
// ) -> Vec<RGBSum> {
//     let n = image.width() / chunk_width;
//     let mut rgbs: Vec<RGBSum> = Vec::new();
//     for idx in 0 .. n {
//         let start = idx * chunk_width;
//         let stop = start + chunk_width;
//         let mut rgb_sum = RGBSum::zero();
//         for x in start .. stop {
//             let rgb = image.get_pixel(x, row);
//             rgb_sum.aggregate(&rgb);
//         }
//         rgbs.push(rgb_sum);
//     }
//     return rgbs;
// }


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


// struct MyUriArg {
//     url: Uri<String>
// }

// impl str::FromStr for MyUriArg {
    
// }


// let default_uri: Uri<String> = Uri::parse("/home/zottel/Pictures/atze.jpg").unwrap();
// let default_url: Uri<String> = Uri::builder().path();

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

    let image = match get_image(&args.image_url) {
        Ok(i) => i,
        Err(error) => panic!("Unable to open image for uri {}: {:?}", args.image_url, error)
    };
    
    // let buf = image.into_rgb8();
    let buf = match image.as_rgb8() {
        Some(b) => b,
        None => panic!("Not an RGB image")
    };
    let t2 = get_time();

    let chunks_x = args.x_chunks;
    let chunks_y = args.y_chunks;

    let n_x = image.width() as usize / chunks_x;
    let n_y = image.height() as usize / chunks_y;
    let n = chunks_x as u64 * chunks_y as u64;

    let mut rgbs = vec![RGBSum::zero(); n_x * n_y];

    for y_chunk in 0 .. n_y {
        let start = y_chunk * chunks_y;
        let stop = start + chunks_y;
        // for x in 0 .. n_x as usize {
        //     rgbs[x].set_zero();
        // };

        let mut slice = &mut rgbs[y_chunk * n_x .. y_chunk * n_x + n_x];
        for y in start .. stop {
            // let lrgbs = sum_chunks(buf, chunks_x as u32, y as u32);
            // for x in 0 .. n_x {
            //     rgbs[x].add(&lrgbs[x]);
            // }
            sum_chunks_inplace(&buf, chunks_x as u32, y as u32, slice);
        }
        for rgb in slice {
            rgb.div(n as u64);
        }
    }
    let t3 = get_time();

    // let mut print_str = String::new();
    
    for y_chunk in 0 .. n_y {
        let mut rgb_count = RgbCount::invalid();
        let slice = &rgbs[y_chunk * n_x .. y_chunk * n_x + n_x];
        // for y in start .. sntop {
        // let rgb = &rgbs[x];
        for rgb in slice {
            if rgb_count.is_valid() {
                if rgb_count.is_same_rgb(rgb) {
                    rgb_count.incr();
                } else {
                    let c = RGB(rgb_count.rgb_sum.r as u8, rgb_count.rgb_sum.g as u8, rgb_count.rgb_sum.b as u8);
                    print!("{}", c.paint("$".repeat(rgb_count.count)));
                    // print_str = format!("{}{}", print_str, c.paint("$".repeat(rgb_count.count)));
                    rgb_count.set_rgb(rgb.clone());
                }
            } else {
                rgb_count.set_rgb(rgb.clone());
            }
        }
        let c = RGB(rgb_count.rgb_sum.r as u8, rgb_count.rgb_sum.g as u8, rgb_count.rgb_sum.b as u8);
        print!("{}", c.paint("$".repeat(rgb_count.count)));
        print!("\n");
        // print_str = format!("{}{}\n", print_str, c.paint("$".repeat(rgb_count.count)));
    }
    // print!("{}", print_str);
    let t4 = get_time();

    // println!("args: {:#?} img: {:#?} transform: {:#?} print: {:#?}", t1 - t0, t2 - t1, t3 - t2, t4 - t3);
}
