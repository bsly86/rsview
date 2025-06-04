use std::io::{BufReader, BufRead};
use std::fs::File;
use serde::Deserialize;
use std::{fs, path::Path};

// Obj Parser //

#[allow(dead_code)]
pub struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Option<Vec<[f32; 3]>>,
}

pub fn parse_obj(file_path: &str) -> Result<Mesh, String> {
    // function for parsing obj files at the simplest level, mesh data only
    let file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut normals = Vec::new();

    for line in reader.lines() {
        
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        let tokens: Vec<&str> = line.split_whitespace().collect();

        match tokens.get(0) {
             Some(&"v") => {
                if tokens.len() < 4 {
                    continue;
                }
                
                let x = tokens[1].parse().map_err(|_| "Invalid vertex x")?;
                let y = tokens[2].parse().map_err(|_| "Invalid vertex y")?;
                let z = tokens[3].parse().map_err(|_| "Invalid vertex z")?;
                vertices.push([x, y, z]);
             }
             Some(&"vn") => {
                if tokens.len() < 4 {
                    continue;
                }

                let x = tokens[1].parse().map_err(|_| "Invalid normal x")?;
                let y = tokens[2].parse().map_err(|_| "Invalid normal y")?;
                let z = tokens[3].parse().map_err(|_| "Invalid normal z")?;
                normals.push([x, y, z]);
             }
             Some(&"f") => {
                // Parse all face indices first
                let face_indices: Vec<u32> = (1..tokens.len())
                    .filter_map(|i| {
                        let index_str = tokens[i];
                        index_str
                            .split('/')
                            .next()
                            .and_then(|s| s.parse::<u32>().ok())
                            .map(|idx| idx - 1)
                    })
                    .collect();

                if face_indices.len() >= 3 {

                    if face_indices.len() == 3 {
                        indices.extend_from_slice(&face_indices);
                    }

                    else if face_indices.len() == 4 {

                        indices.push(face_indices[0]);
                        indices.push(face_indices[1]);
                        indices.push(face_indices[2]);
                        
                        indices.push(face_indices[0]);
                        indices.push(face_indices[2]);
                        indices.push(face_indices[3]);
                    }

                    else {
                        for i in 1..(face_indices.len() - 1) {
                            indices.push(face_indices[0]);
                            indices.push(face_indices[i]);
                            indices.push(face_indices[i + 1]);
                        }
                    }
                }
             }
             _ => {}
        }
    }

    println!("OBJ Parser: Loaded {} vertices, {} indices ({} triangles)", 
             vertices.len(), indices.len(), indices.len() / 3);

    Ok(Mesh {
        vertices,
        indices,
        normals: if normals.is_empty() { None } else { Some(normals) },
    })
}

// GLTF parser //

#[derive(Debug, Deserialize)]
pub struct GltfFile {
    buffers: Vec<Buffer>,
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<BufferView>,
    accessors: Vec<Accessor>,
    meshes: Vec<GltfMesh>,
}

#[derive(Debug, Deserialize)]
pub struct Buffer {
    uri: String,
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

#[derive(Debug, Deserialize)]
struct BufferView {
    buffer: usize,
    #[serde(rename = "byteOffset")]
    byte_offset: Option<usize>,
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

#[derive(Debug, Deserialize)]
struct Accessor {
    #[serde(rename = "bufferView")]
    buffer_view: usize, 
    #[serde(rename = "byteOffset")]
    byte_offset: Option<usize>,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    accessor_type: String,
}

#[derive(Debug, Deserialize)]
struct GltfMesh {
    primitives: Vec<Primitive>,
}

#[derive(Debug, Deserialize)]
struct Primitive {
    attributes: std::collections::HashMap<String, usize>,
    indices: Option<usize>,
}

pub fn parse_gltf(file_path: &str) -> Result<Mesh, String> {
    let path = Path::new(file_path);

    let json_text = fs::read_to_string(file_path)
                                .map_err(|e| format!("Failed to read gLTF file: {}", e))?;
    let gltf: GltfFile = serde_json::from_str(&json_text)
                            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let base_dir = path.parent()
                            .ok_or("Failed to get base directory")?;

    let buffer_uri = &gltf.buffers[0].uri;
    let buffer_path = base_dir.join(buffer_uri);
    let buffer_data = fs::read(&buffer_path)
        .map_err(|e| format!("Failed to read buffer: {}", e))?;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    if let Some(mesh) = gltf.meshes.first() {
        if let Some(prim) = mesh.primitives.first() {
            if let Some(&pos_index) = prim.attributes.get("POSITION") {
                let pos_accessor = &gltf.accessors[pos_index];
                let view = &gltf.buffer_views[pos_accessor.buffer_view];
                let offset = view.byte_offset.unwrap_or(0) + pos_accessor.byte_offset.unwrap_or(0);

                for i in 0..pos_accessor.count {
                    let start = offset + i * 12;
                    let x = f32::from_le_bytes(buffer_data[start..start + 4]
                        .try_into()
                        .unwrap());
                    let y = f32::from_le_bytes(buffer_data[start + 4..start + 8]
                        .try_into()
                        .unwrap());
                    let z = f32::from_le_bytes(buffer_data[start + 8..start + 12]
                        .try_into()
                        .unwrap());
                    vertices.push([x, y, z]);
                }
            }

            if let Some(idx_index) = prim.indices {
                let idx_accessor = &gltf.accessors[idx_index];
                let view = &gltf.buffer_views[idx_accessor.buffer_view];
                let offset = view.byte_offset.unwrap_or(0) + idx_accessor.byte_offset.unwrap_or(0);

                let component_size = match idx_accessor.component_type {
                    5121 => 1, // UNSIGNED_BYTE
                    5123 => 2, // UNSIGNED_SHORT
                    5125 => 4, // UNSIGNED_INT
                    _ => return Err("Unsupported index component type".into()),
                };

                for i in 0..idx_accessor.count {
                    let start = offset + i * component_size;
                    let index = match component_size {
                        1 => buffer_data[start] as u32,
                        2 => u16::from_le_bytes(buffer_data[start..start + 2].try_into().unwrap()) as u32,
                        4 => u32::from_le_bytes(buffer_data[start..start + 4].try_into().unwrap()),
                        _ => return Err("Unexpected index size".into()),
                    };
                    indices.push(index);
                }
            }
        }
    }

    println!("GLTF Parser: Loaded {} vertices, {} indices ({} triangles)", 
             vertices.len(), indices.len(), indices.len() / 3);

    Ok(Mesh {
        vertices,
        indices,
        normals: None,
    })
}