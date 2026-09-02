# Recover Canonical Sources From Public X Article Cards

Use this when an `x.com/<handle>/status/<id>` post exposes an Article card while
the article route opens a login wall. Inspect anchors inside the public status
post before navigation. Article cards commonly target `/i/article/<id>`.

Authors may cross-post the same article on LinkedIn or another public profile.
Inspect the public DOM and decode the `url` parameter from LinkedIn redirect
links, then validate the title and body at the author-controlled canonical URL.

Treat the X status and the author's canonical page as primary sources. When a
canonical page remains unavailable, cite the public status and mark the body as
inaccessible. Keep credentials and login-wall bypass attempts outside the flow.
