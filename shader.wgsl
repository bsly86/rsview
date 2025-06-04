struct Uniforms {
    mvp: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) frag_pos: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.mvp * vec4(pos, 1.0);
    out.frag_pos = pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {

    let dx = dpdx(in.frag_pos);
    let dy = dpdy(in.frag_pos);
    let face_normal = normalize(cross(dx, dy));
    
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let n_dot_l = max(dot(face_normal, light_dir), 0.0);
    
    let ambient = 0.3;
    let diffuse = 0.7 * n_dot_l;
    let brightness = ambient + diffuse;
    
    let base_color = vec3<f32>(0.8, 0.8, 0.8);
    let final_color = base_color * brightness;
    
    return vec4(final_color, 1.0);
}