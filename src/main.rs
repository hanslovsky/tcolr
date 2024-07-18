use ansi_term::Colour::RGB;
use clap::Parser;
use image::DynamicImage;
use image::ImageBuffer;
use image::ImageError;
use image::Rgb;
use image::io::Reader;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

fn get_image(path: String) -> Result<DynamicImage, ImageError> {
    return Reader::open(path)?.decode();
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

pub trait AddRGB {
    fn add_rgb(&mut self, rgb: &Rgb<u8>);
}


impl AddRGB for RGBSum {
    fn add_rgb(&mut self, rgb: &Rgb<u8>) {
        self.r += rgb[0] as u64;
        self.g += rgb[1] as u64;
        self.b += rgb[2] as u64;
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


fn sum_chunks(
    image: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    chunk_width: u32,
    row: u32
) -> Vec<RGBSum> {
    let n = image.width() / chunk_width;
    let mut rgbs: Vec<RGBSum> = Vec::new();
    for idx in 0 .. n {
        let start = idx * chunk_width;
        let stop = start + chunk_width;
        let mut rgb_sum = RGBSum::zero();
        for x in start .. stop {
            let rgb = image.get_pixel(x, row);
            rgb_sum.add_rgb(rgb);
        }
        rgbs.push(rgb_sum);
    }
    return rgbs;
}


fn sum_chunks_inplace(
    image: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    chunk_width: u32,
    row: u32,
    target: &mut [RGBSum]
) {
    let n = image.width() / chunk_width;
    for idx in 0 .. n {
        let start = idx * chunk_width;
        let stop = start + chunk_width;
        for x in start .. stop {
            let rgb = image.get_pixel(x, row);
            target[idx as usize].add_rgb(rgb);
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {

    #[arg(short, long, default_value_t = String::from("/home/zottel/Pictures/atze.jpg"))]
    path: String,

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
    
    let image = match get_image(args.path) {
        Ok(i) => i,
        Err(error) => panic!("Unable to open image: {:?}", error)
    };
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
            sum_chunks_inplace(buf, chunks_x as u32, y as u32, slice);
        }
        for rgb in slice {
            rgb.div(n as u64);
        }
    }
    let t3 = get_time();
    for y_chunk in 0 .. n_y {
        let start = y_chunk * chunks_y;
        let stop = start + chunks_y;
        let slice = &rgbs[y_chunk * n_x .. y_chunk * n_x + n_x];
        // for y in start .. stop {
        // let rgb = &rgbs[x];
        for rgb in slice {
            let c = RGB(rgb.r as u8, rgb.g as u8, rgb.b as u8);
            print!("{}", c.paint("$"));
        }
        print!("\n");
    }
    let t4 = get_time();

    println!("args: {:#?} img: {:#?} transform: {:#?} print: {:#?}", t1 - t0, t2 - t1, t3 - t2, t4 - t3);
}
