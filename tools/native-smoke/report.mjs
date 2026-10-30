// Reject callback-only, stale-host and unusable-window reports.
export function validateNativeFrame(frame, host = process) {
  const platform = { darwin: 'macos', win32: 'windows', linux: 'linux' }[host.platform];
  const arch = { arm64: 'aarch64', x64: 'x86_64' }[host.arch];
  if (!platform || !arch || !frame) throw new Error('Invalid native frame report host or missing frame');
  const failures = [];
  if (frame.platform !== platform) failures.push(`platform=${frame.platform} expected=${platform}`);
  if (frame.arch !== arch) failures.push(`arch=${frame.arch} expected=${arch}`);
  if (frame.renderedFrame !== true) failures.push(`renderedFrame=${frame.renderedFrame}`);
  if (!Number.isSafeInteger(frame.submittedFrames) || frame.submittedFrames < 1) failures.push(`submittedFrames=${frame.submittedFrames}`);
  if (!Number.isFinite(frame.width) || frame.width < 960) failures.push(`width=${frame.width}`);
  if (!Number.isFinite(frame.height) || frame.height < 600) failures.push(`height=${frame.height}`);
  if (failures.length) throw new Error(`Invalid native frame report: ${failures.join(', ')}`);
  return { ...frame, status: 'passed', physicalDesktopAcceptance: 'not_evaluated' };
}
