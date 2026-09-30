// Tiny path router: every demo has its own URL (/vault, /pool...) to share or project.
export const route = $state({ path: location.pathname })

window.addEventListener('popstate', () => (route.path = location.pathname))

export function navigate(path: string) {
  if (path === route.path) return
  history.pushState(null, '', path)
  route.path = path
  window.scrollTo(0, 0)
}
