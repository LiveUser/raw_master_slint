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

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let ui_handle = ui.as_weak();
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
    });
    ui.on_select_lut(move |selected_lut:LUT| {
        let settings:Entry =Entry { 
            db_path:  String::from("./settings"),
        };
        let _ = settings.init();
        let _ = settings.select().insert("selected-lut".to_string(), DbData::Text(selected_lut.full_path.to_string()));
    });
    ui.on_process_raw_images(move || {
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
        .pick_file(|result| {
            if let Ok(Some(file)) = result {
                let settings:Entry = Entry { 
                    db_path:  String::from("./settings"),
                };
                let _ = settings.init();
                let mut lut_path:String = String::new();
                match settings.select().view() {
                    Err(_) => {

                    }
                    Ok(mut object) => {
                        match object.get("selected-lut".to_string()) {
                            Dynamic::String(lut_full_path) => {
                                lut_path = lut_full_path;
                            }
                            _ => {

                            }
                        }
                    }
                }
                //Read Raw Image
                match rawloader::decode_file(file.path().unwrap()) {
                    Err(error) => {
                        println!("{:?}",error);
                    }
                    Ok(raw_image) => {
                        let mut pipeline = Pipeline::new_from_source(imagepipe::ImageSource::Raw(raw_image.clone())).unwrap();
                        let decoded = pipeline.output_16bit(None).unwrap();
                        let rgb: Vec<u16> = decoded.data;
                        //Read and apply lut-----------------------------
                        
                        //Save as JPEG XL---------------------------------
                        // Build the encoder
                        let config = EncodeConfig::default();
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
    ui.run()?;

    Ok(())
}
