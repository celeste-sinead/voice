# A more reliable (?) work-around for the nvidia vulkan bug
# See: https://github.com/iced-rs/iced/issues/2314
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json
export VK_LAYER_PATH=/usr/share/vulkan/explicit_layer.d