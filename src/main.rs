// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use robius_file_picker::FileDialog;
use dart_io::{Directory, File, FileSystemEntity};
use objective_db::{Entry,DbData,DbError,DbObject};
use easy_bson::Dynamic;
use rawloader::{RawLoader,RawImageData};
use power_plant::generate_unique_alphanumeric;
use jixel::{EncodeConfig, encode_image_16bit};
use imagepipe::Pipeline;
use slint::{ModelRc, ToSharedString, VecModel};
use oximedia_lut::{Lut3d, LutInterpolation, ColorSpace};

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
        let _ = FileDialog::new()
        .add_filter("LUT", &["cube", "3dl", "csp", "look", "lut", "clf", "aml"])
        .pick_file(|result| {
            if let Ok(Some(file)) = result {
                //Store the LUT file
                let lut_file:File = File { 
                    full_path: format!("./luts/{}",file.file_name().unwrap()),
                };
                if(!lut_file.exists()){
                    lut_file.create_sync();
                }
                lut_file.write_as_bytes(file.read_bytes().unwrap());
            }
        });
        if let Some(ui) = ui_handle.upgrade() {
            ui.set_luts(get_luts());
        }
    });
    ui.on_select_lut(move |selected_lut:LUT| {
        let settings:Entry = Entry { 
            db_path:  String::from("./settings"),
        };
        let _ = settings.init();
        let _ = settings.select().insert("selected-lut".to_string(), DbData::Text(selected_lut.full_path.to_string()));
    });
    ui.on_process_raw_images(move |quality| {
        let _ = FileDialog::new()
        .add_filter("Raw Image", &[
            "mrw",
            "arw",
            "srf",
            "sr2",
            "mef",
            "orf",
            "srw",
            "erf",
            "kdc",
            "dcs",
            "rw2",
            "raf",
            "dcr",
            "dng",
            "pef",
            "crw",
            "iiq",
            "3fr",
            "nrw",
            "nef",
            "mos",
            "cr2",
            "ari",
        ])
        .pick_file(move |result| {
            if let Ok(Some(file)) = result {
                //Read Raw Image
                match rawloader::decode_file(file.path().unwrap()) {
                    Err(error) => {
                        println!("{:?}",error);
                    }
                    Ok(raw_image) => {
                        let mut pipeline = Pipeline::new_from_source(imagepipe::ImageSource::Raw(raw_image.clone())).unwrap();
                        let decoded = pipeline.output_16bit(None).unwrap();
                        let mut rgb: Vec<u16> = decoded.data;
                        //Read and apply lut-----------------------------
                        let lut_path:String = get_selected_lut();
                        if lut_path != "".to_string(){
                            let lut = Lut3d::from_file(lut_path).unwrap();
                            apply_lut_to_16bit_buffer(&lut, &mut rgb);
                        }
                        //Save as JPEG XL---------------------------------
                        // Build the encoder and pass on the exif data
                        let config = EncodeConfig::default()
                        .with_quality(quality)
                        .with_lossless(true);
                        //.with_exif();
                        let jxl_bytes = encode_image_16bit(&rgb, decoded.width, decoded.height, &config).unwrap();
                        let new_image_path:String = format!("./output_images/{}.jxl", generate_unique_alphanumeric(40));
                        let new_file:File = File { 
                            full_path: new_image_path,
                        };
                        if !new_file.exists(){
                            new_file.create_sync();
                        }
                        new_file.write_as_bytes(jxl_bytes);
                    }
                }
            }
        });
    });
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
