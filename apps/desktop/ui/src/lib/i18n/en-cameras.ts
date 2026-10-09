// Camera page and indicator texts (plan v6, C3).
export const enCameras = {
  'cam.lead':
    'Built-in cameras and cameras connected over USB. A camera opens only when someone asks for an image and goes to sleep by itself. Network cameras (IP, Wi-Fi), image description and person recognition are planned.',
  'cam.list': 'Your cameras',
  'cam.search': 'Look for cameras',
  'cam.loading': 'Looking for cameras…',
  'cam.none': 'I can see no camera',
  'cam.noneHint':
    'Plug in a USB camera and choose “Look for cameras”. For a built-in camera, check in Windows privacy settings that apps may use it.',
  'cam.masterOff': 'Cameras are off — none will open. Turn them on with the switch above.',
  'cam.preview': 'Preview',
  'cam.stopPreview': 'Stop preview',
  'cam.previewAlt': 'Image from camera: {name}',
  'cam.rename': 'Rename camera {name}',
  'cam.nameLabel': 'Camera name',
  'cam.saveName': 'Save name',
  'cam.enable': 'Camera {name} enabled',
  'cam.on': 'on',
  'cam.off': 'off',
  'cam.kind.usb': 'USB',
  'cam.kind.network': 'network',
  'cam.state.asleep': 'asleep',
  'cam.state.opening': 'opening…',
  'cam.state.live': 'live',
  'cam.state.failed': 'error',
  'cam.state.unplugged': 'unplugged',
  'cam.privacy': 'Privacy',
  'cam.privacy.record':
    'The preview stores nothing — a frame lives in memory only while it is shown. A photo you attach to a message yourself stays in the conversation like any attachment.',
  'cam.privacy.sleep':
    'Without a preview a camera goes to sleep after several seconds; a disabled camera never opens.',
  'cam.privacy.indicator': 'While any camera is live, a red camera sign is lit in the top bar.',
  'cam.privacy.next':
    'Use the camera button in the message field to attach a photo and ask “describe what you see” — this needs a model that accepts images. Agents do not reach for cameras on their own; that is the next step of the plan.',
  'cam.snap': 'Attach a photo from camera: {name}',
  'cam.snapPick': 'Attach a photo from a camera',
  'cam.indicator': {
    one: '{n} camera is live: {names}. Open camera settings',
    other: '{n} cameras are live: {names}. Open camera settings',
  },
} as const;
