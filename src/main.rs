// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{error::Error};
use dart_io::{Directory, File, FileSystemEntity};
use objective_db::{Entry,DbData};
use easy_bson::Dynamic;
use power_plant::generate_unique_alphanumeric;
use jixel::{EncodeConfig, encode_image_16bit};
use slint::{ModelRc, ToSharedString, VecModel};
use oximedia_lut::{Lut3d, LutInterpolation};
use rfd::FileDialog;
use std::path::Path;
use rawler::{self, imgop::develop::RawDevelop, decoders::RawDecodeParams, rawsource::RawSource};
use little_exif::{metadata::Metadata,filetype::FileExtension,exif_tag::ExifTag};
use little_exif::rational::{iR64, uR64};

slint::include_modules!();

pub fn apply_lut_to_16bit_buffer(lut:&Lut3d, rgb_buffer: &mut [u16]) {
    const INV_MAX: f64 = 1.0 / 65535.0;

    for pixel in rgb_buffer.chunks_exact_mut(3) {
        // 1. Convert u16 [0, 65535] to normalized f64 [0.0, 1.0]
        let input = [
            pixel[0] as f64 * INV_MAX,
            pixel[1] as f64 * INV_MAX,
            pixel[2] as f64 * INV_MAX,
        ];

        // 2. Apply LUT using high-quality tetrahedral interpolation
        let output = lut.apply(&input, LutInterpolation::Tetrahedral);

        // 3. Quantize back to u16 [0, 65535] with safety clamping
        pixel[0] = (output[0].clamp(0.0, 1.0) * 65535.0).round() as u16;
        pixel[1] = (output[1].clamp(0.0, 1.0) * 65535.0).round() as u16;
        pixel[2] = (output[2].clamp(0.0, 1.0) * 65535.0).round() as u16;
    }
}

fn get_selected_lut() -> String{
    let settings:Entry = Entry { 
        db_path:  String::from("./settings"),
    };
    let _ = settings.init();
    match settings.select().view() {
        Err(_) => {
            return "".to_string();
        }
        Ok(mut object) => {
            match object.get("selected-lut".to_string()) {
                Dynamic::String(lut_full_path) => {
                    return  lut_full_path;
                }
                _ => {
                    return "".to_string();
                }
            }
        }
    }
}
//Main------------------------------------------------------------------
fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let ui_handle = ui.as_weak();

    //Load Stored LUTs on app start
    fn get_luts() -> ModelRc<LUT>{
        let mut luts:Vec<LUT> = Vec::new();
        let luts_dir:Directory = Directory { 
            full_path: "./luts".to_string(),
         };
         let contents = luts_dir.list_contents();
         for content in contents{
            match content {
                FileSystemEntity::Directory(_) => {

                }
                FileSystemEntity::File(file) => {
                    let last_slash = file.full_path.rfind("/").unwrap() + 1;
                    let file_name:&str = &file.full_path[last_slash..];
                    luts.push(LUT { 
                        full_path: file.full_path.to_shared_string(), 
                        name: String::from(file_name.to_shared_string()).to_shared_string(),
                    });
                }
            }
         }
        return ModelRc::new(VecModel::from(luts));
    }
    //App Startup---------------------------------------------
    ui.set_luts(get_luts());
    ui.set_selected_lut(get_selected_lut().to_shared_string());
    //--------------------------------------------------------

    ui.on_store_lut(move || {
        let files = FileDialog::new()
        .add_filter("LUT", &[
            "cube",
            "3dl",
            "csp",
            "look",
            "lut",
            "clf",
            "aml",
            "CUBE",
            "3DL",
            "CSP",
            "LOOK",
            "LUT",
            "CLF",
            "AML"
        ]).pick_files().unwrap();
        for file in files.iter(){
            let file_path:String = file.to_str().unwrap().to_string();
            let last_slash_index: usize = file_path.rfind("/").unwrap();
            let file_name:String = file_path[(last_slash_index + 1)..].to_string();
            //Store the LUT file
            let lut_file:File = File { 
                full_path: format!("./luts/{}",file_name),
            };
            if !lut_file.exists(){
                lut_file.create_sync();
            }
            let selected_file:File = File { 
                full_path: file_path,
            };
            lut_file.write_as_bytes(selected_file.read_as_bytes());
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_luts(get_luts());
            }
        }
    });
    ui.on_select_lut(move |selected_lut:LUT| {
        let settings:Entry = Entry { 
            db_path:  String::from("./settings"),
        };
        let _ = settings.init();
        let _ = settings.select().insert("selected-lut".to_string(), DbData::Text(selected_lut.full_path.to_string()));
    });
    ui.on_process_raw_images({
    let ui_handle = ui.as_weak();
    move |quality, lossless_compression| {
        let files = FileDialog::new()
        .add_filter("RAW Image", &[
          "mrw", "MRW",
          "arw", "ARW",
          "srf", "SRF",
          "sr2", "SR2",
          "mef", "MEF",
          "orf", "ORF",
          "srw", "SRW",
          "erf", "ERF",
          "kdc", "KDC",
          "dcs", "DCS",
          "rw2", "RW2",
          "raf", "RAF",
          "dcr", "DCR",
          "dng", "DNG",
          "pef", "PEF",
          "crw", "CRW",
          "iiq", "IIQ",
          "3fr", "3FR",
          "nrw", "NRW",
          "nef", "NEF",
          "mos", "MOS",
          "cr2", "CR2",
          "cr3", "CR3",
          "ari", "ARI",
          "cap", "CAP",
          "eip", "EIP",
          "fff", "FFF",
          "ptx", "PTX",
          "pxn", "PXN",
          "r3d", "R3D",
          "rwl", "RWL",
          "x3f", "X3F",
          "bay", "BAY",
          "cbq", "CBQ",
          "data", "DATA",
          "drf", "DRF",
          "dsc", "DSC"
        ]).pick_files().unwrap();
        let ui_handle = ui_handle.clone();
        let total = files.len() as i32;
        let _ = ui_handle.upgrade_in_event_loop(move |ui| {
            let images_being_processed = ui.get_files_being_processed() + total;
            ui.set_files_being_processed(images_being_processed);
        });
        std::thread::spawn(move || {
            for file in files{
                let file_path:String = file.to_str().unwrap().to_string();
                //Load RawImage
                let path = Path::new(&file_path);
                let raw_image = match rawler::decode_file(&path) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to decode {:?}: {e}", path);
                        let _ = ui_handle.upgrade_in_event_loop(|ui| {
                            ui.set_files_being_processed(ui.get_files_being_processed() - 1);
                        });
                        continue;
                    }
                };
                //Pass onto the pipeline to parse rgb
                let developer = RawDevelop::default();
                let developed = developer.develop_intermediate(&raw_image).unwrap();
                let dynamic_image = developed.to_dynamic_image().unwrap();
                let mut rgb: Vec<u16> = dynamic_image.as_rgb16().unwrap().clone().to_vec();

                //Read and apply lut-----------------------------
                let lut_path:String = get_selected_lut();
                if lut_path != "".to_string(){
                    let lut = Lut3d::from_file(lut_path).unwrap();
                    apply_lut_to_16bit_buffer(&lut, &mut rgb);
                }
                //Save as JPEG XL---------------------------------
                //Raw file metadata----
                let src = RawSource::new(&path).unwrap();
                let decoder = rawler::get_decoder(&src).unwrap();
                let meta = decoder.raw_metadata(&src, &RawDecodeParams::default()).unwrap();
                let exif = &meta.exif;
                //New metadata-------
                let mut file_metadata = Metadata::new();
                file_metadata.set_tag(ExifTag::Make(raw_image.make.clone()));
                file_metadata.set_tag(ExifTag::Model(raw_image.model.clone()));
                file_metadata.set_tag(ExifTag::ImageWidth(vec![dynamic_image.width()]));
                file_metadata.set_tag(ExifTag::ImageHeight(vec![dynamic_image.height()]));
                file_metadata.set_tag(ExifTag::Orientation(vec![1]));

                if let Some(v) = &exif.artist { file_metadata.set_tag(ExifTag::Artist(v.clone())); }
                if let Some(v) = &exif.copyright { file_metadata.set_tag(ExifTag::Copyright(v.clone())); }
                if let Some(v) = &exif.lens_make { file_metadata.set_tag(ExifTag::LensMake(v.clone())); }
                if let Some(v) = &exif.lens_model { file_metadata.set_tag(ExifTag::LensModel(v.clone())); }
                if let Some(v) = &exif.serial_number { file_metadata.set_tag(ExifTag::SerialNumber(v.clone())); }
                if let Some(v) = &exif.date_time_original { file_metadata.set_tag(ExifTag::DateTimeOriginal(v.clone())); }
                if let Some(v) = &exif.create_date { file_metadata.set_tag(ExifTag::CreateDate(v.clone())); }

                if let Some(r) = &exif.exposure_time {
                    file_metadata.set_tag(ExifTag::ExposureTime(vec![uR64 { nominator: r.n, denominator: r.d }]));
                }
                if let Some(r) = &exif.fnumber {
                    file_metadata.set_tag(ExifTag::FNumber(vec![uR64 { nominator: r.n, denominator: r.d }]));
                }
                if let Some(r) = &exif.focal_length {
                    file_metadata.set_tag(ExifTag::FocalLength(vec![uR64 { nominator: r.n, denominator: r.d }]));
                }
                if let Some(r) = &exif.aperture_value {
                    file_metadata.set_tag(ExifTag::ApertureValue(vec![uR64 { nominator: r.n, denominator: r.d }]));
                }
                if let Some(r) = &exif.brightness_value {
                    file_metadata.set_tag(ExifTag::BrightnessValue(vec![iR64 { nominator: r.n, denominator: r.d }]));
                }
                if let Some(r) = &exif.exposure_bias {
                    file_metadata.set_tag(ExifTag::ExposureCompensation(vec![iR64 { nominator: r.n, denominator: r.d }]));
                }

                if let Some(v) = exif.iso_speed { file_metadata.set_tag(ExifTag::ISO(vec![v as u16])); }
                if let Some(v) = exif.exposure_program { file_metadata.set_tag(ExifTag::ExposureProgram(vec![v])); }
                if let Some(v) = exif.metering_mode { file_metadata.set_tag(ExifTag::MeteringMode(vec![v])); }
                if let Some(v) = exif.flash { file_metadata.set_tag(ExifTag::Flash(vec![v])); }
                if let Some(v) = exif.white_balance { file_metadata.set_tag(ExifTag::WhiteBalance(vec![v])); }
                // Slice off any JPEG framing (ff e1, length, "Exif\0\0") so only the bare TIFF block remains
                let exif_bytes = file_metadata.as_u8_vec(FileExtension::JPEG).unwrap();
                let start = exif_bytes
                    .windows(4)
                    .position(|w| w == b"II*\0" || w == b"MM\0*")
                    .unwrap_or(0);
                let tiff_block = exif_bytes[start..].to_vec();
                // Build the encoder and pass on the exif data
                let config = EncodeConfig::default()
                .with_exif(tiff_block)
                .with_quality(quality)
                .with_lossless(lossless_compression);
                //.with_exif();
                let jxl_bytes = encode_image_16bit(&rgb, dynamic_image.width() as usize, dynamic_image.height() as usize, &config).unwrap();
                let new_image_path:String = format!("./output_images/{}.jxl", generate_unique_alphanumeric(40));
                let new_file:File = File { 
                    full_path: new_image_path,
                };
                if !new_file.exists(){
                    new_file.create_sync();
                }
                new_file.write_as_bytes(jxl_bytes);
                //Update the UI
                let _ = ui_handle.upgrade_in_event_loop(|ui| {
                    let images_being_processed = ui.get_files_being_processed() - 1;
                    ui.set_files_being_processed(images_being_processed);
                });
            }
        });
    }});
    ui.on_clear_lut_selection(move || {
        let settings:Entry = Entry { 
            db_path:  String::from("./settings"),
        };
        let _ = settings.init();
        let _ = settings.select().insert("selected-lut".to_string(), DbData::Text("".to_string()));
    });
    ui.run()?;

    Ok(())
}
