//! The grid pass: [`GridSubmission`]s drawn as patches of their planes,
//! the lines worked out per pixel in `grid.frag`.

use ash::vk;
use std::mem::size_of;

use crate::mesh::NO_CLIP;
use crate::{GRID_FRAG_SPV, GRID_VERT_SPV, GridSubmission, RenderError, create_shader_module};

/// `grid.vert`'s and `grid.frag`'s push-constant block.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridPushConstants {
    view_proj: [[f32; 4]; 4],
    camera_step: [f32; 4],
    clip_plane: [f32; 4],
    anchor_radius: [f32; 4],
    u_minor: [f32; 4],
    v_major: [f32; 4],
    center_axis: [f32; 4],
    forward_near: [f32; 4],
    color_far: [f32; 4],
    u_axis_color: [f32; 4],
    v_axis_color: [f32; 4],
}

const GRID_PUSH_SIZE: u32 = size_of::<GridPushConstants>() as u32;

impl GridPushConstants {
    /// The block for `grid`. The plane coordinates the shader reads are
    /// measured from a node of the coarsest lines next to the patch's
    /// centre rather than from the origin, so they stay small, and the
    /// lines sharp, however far from the origin the view is.
    pub(crate) fn new(
        grid: &GridSubmission,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        clip_plane: Option<[f32; 4]>,
    ) -> Self {
        let dot = |a: [f32; 3], b: [f32; 3]| -> f64 {
            a.iter()
                .zip(b)
                .map(|(a, b)| f64::from(*a) * f64::from(b))
                .sum()
        };
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let coarsest = f64::from(grid.step) * 100.0;
        let to_center = sub(grid.center, grid.origin);
        let node = [
            (dot(to_center, grid.u) / coarsest).round() * coarsest,
            (dot(to_center, grid.v) / coarsest).round() * coarsest,
        ];
        let anchor: [f32; 3] = std::array::from_fn(|i| {
            (f64::from(grid.origin[i])
                + f64::from(grid.u[i]) * node[0]
                + f64::from(grid.v[i]) * node[1]) as f32
        });
        let with = |xyz: [f32; 3], w: f32| [xyz[0], xyz[1], xyz[2], w];
        Self {
            view_proj,
            camera_step: with(camera_pos, grid.step),
            clip_plane: clip_plane.unwrap_or(NO_CLIP),
            anchor_radius: with(anchor, grid.radius),
            u_minor: with(grid.u, grid.minor_alpha),
            v_major: with(grid.v, grid.major_alpha),
            center_axis: with(grid.center, grid.axis_alpha),
            forward_near: with(grid.forward, grid.depth_range[0]),
            color_far: with(grid.color, grid.depth_range[1]),
            u_axis_color: with(grid.u_axis_color, -node[0] as f32),
            v_axis_color: with(grid.v_axis_color, -node[1] as f32),
        }
    }
}

pub(crate) struct GridPipeline {
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl GridPipeline {
    pub(crate) fn new(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        msaa_samples: vk::SampleCountFlags,
    ) -> Result<Self, RenderError> {
        let ranges = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(GRID_PUSH_SIZE)];
        let layout_info = vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&ranges);
        let layout = unsafe { device.create_pipeline_layout(&layout_info, None) }?;
        let pipeline = match create_pipeline(device, render_pass, layout, msaa_samples) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                unsafe { device.destroy_pipeline_layout(layout, None) };
                return Err(err);
            }
        };
        Ok(Self { layout, pipeline })
    }

    pub(crate) fn set_render_pass(
        &mut self,
        device: &ash::Device,
        render_pass: vk::RenderPass,
        msaa_samples: vk::SampleCountFlags,
    ) -> Result<(), RenderError> {
        unsafe { device.destroy_pipeline(self.pipeline, None) };
        self.pipeline = create_pipeline(device, render_pass, self.layout, msaa_samples)?;
        Ok(())
    }

    /// Draw every grid; the caller has set the viewport and scissor.
    pub(crate) fn draw(
        &self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        grids: &[GridSubmission],
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        clip_plane: Option<[f32; 4]>,
    ) {
        if grids.is_empty() {
            return;
        }
        unsafe {
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
        }
        for grid in grids {
            if grid.step <= 0.0 || grid.radius <= 0.0 {
                continue;
            }
            let pc = GridPushConstants::new(grid, view_proj, camera_pos, clip_plane);
            unsafe {
                let bytes = std::slice::from_raw_parts(
                    &pc as *const GridPushConstants as *const u8,
                    GRID_PUSH_SIZE as usize,
                );
                device.cmd_push_constants(
                    command_buffer,
                    self.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytes,
                );
                device.cmd_draw(command_buffer, 6, 1, 0, 0);
            }
        }
    }

    pub(crate) fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
        }
    }
}

fn create_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    msaa_samples: vk::SampleCountFlags,
) -> Result<vk::Pipeline, RenderError> {
    let vert_module = create_shader_module(device, GRID_VERT_SPV)?;
    let frag_module = match create_shader_module(device, GRID_FRAG_SPV) {
        Ok(module) => module,
        Err(err) => {
            unsafe { device.destroy_shader_module(vert_module, None) };
            return Err(err);
        }
    };
    let entry_name = c"main";
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_module)
            .name(entry_name),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_module)
            .name(entry_name),
    ];
    // The patch's corners come from the vertex index: no vertex input.
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE);
    let multisampling =
        vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(msaa_samples);
    // Behind what is drawn, seen through what is see-through, and never in
    // the way of what draws after it.
    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(false)
        .depth_compare_op(vk::CompareOp::LESS);
    let blend = [vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD)];
    let color_blending = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisampling)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blending)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);
    let result =
        unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) };
    unsafe {
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
    }
    Ok(result.map_err(|(_, err)| RenderError::from(err))?[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(center: [f32; 3], step: f32) -> GridSubmission {
        GridSubmission {
            origin: [0.0; 3],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
            step,
            center,
            radius: 100.0,
            forward: [0.0, 0.0, -1.0],
            depth_range: [0.1, 1000.0],
            color: [1.0; 3],
            minor_alpha: 0.1,
            major_alpha: 0.3,
            u_axis_color: [1.0, 0.0, 0.0],
            v_axis_color: [0.0, 1.0, 0.0],
            axis_alpha: 0.8,
        }
    }

    /// Far from the origin the shader measures from a node of the
    /// coarsest lines, and is told where the axes lie from there.
    #[test]
    fn the_plane_is_measured_from_a_node_next_to_the_patch() {
        let pc = GridPushConstants::new(
            &grid([12_345.0, -6_789.0, 0.0], 1.0),
            [[0.0; 4]; 4],
            [0.0; 3],
            None,
        );
        assert_eq!(pc.anchor_radius[..3], [12_300.0, -6_800.0, 0.0]);
        assert_eq!(pc.u_axis_color[3], -12_300.0);
        assert_eq!(pc.v_axis_color[3], 6_800.0);
    }

    #[test]
    fn a_grid_in_another_plane_keeps_its_node_in_that_plane() {
        let mut g = grid([0.0, 0.0, 0.0], 0.5);
        g.origin = [0.0, 0.0, 7.0];
        g.u = [0.0, 1.0, 0.0];
        g.v = [0.0, 0.0, 1.0];
        g.center = [0.0, 260.0, 7.0 - 140.0];
        let pc = GridPushConstants::new(&g, [[0.0; 4]; 4], [0.0; 3], None);
        assert_eq!(pc.anchor_radius[..3], [0.0, 250.0, 7.0 - 150.0]);
    }
}
