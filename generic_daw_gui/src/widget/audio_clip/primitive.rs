use super::{pipeline::Pipeline, sample::Sample};
use iced_wgpu::wgpu;

#[derive(Clone, Debug, Default)]
pub struct Primitive {
    pub texture: Vec<Sample>,
}

impl iced::widget::shader::Primitive for Primitive {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        storage: &mut iced::widget::shader::Storage,
        _bounds: &iced::Rectangle,
        _viewport: &iced_wgpu::graphics::Viewport,
    ) {
        if !storage.has::<Pipeline>() {
            storage.store(Pipeline::new(device, format));
        }

        let pipeline = storage.get_mut::<Pipeline>().unwrap();

        pipeline.set_texture(device, queue, &self.texture);
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &iced::widget::shader::Storage,
        target: &wgpu::TextureView,
        clip_bounds: &iced::Rectangle<u32>,
    ) {
        let pipeline = storage.get::<Pipeline>().unwrap();

        pipeline.render(target, encoder, *clip_bounds);
    }
}
