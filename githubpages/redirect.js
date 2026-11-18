import routes from "./redirect-routes.js";

const fallback = "https://wiki.shroudforge.net/index.php/Main_Page";
const route = location.hash.slice(1);
const destination = routes[route] || fallback;
const currentUrl = document.querySelector("#old-url");
const destinationLink = document.querySelector("#destination");
const destinationUrl = document.querySelector("#destination-url");
const destinationTitle = document.querySelector("#destination-title");
const routeFlow = document.querySelector(".route-flow");

if (routeFlow) {
  const restartAnimationSequence = () => {
    routeFlow.classList.remove("is-animating");
    void routeFlow.offsetWidth;
    routeFlow.classList.add("is-animating");
  };

  restartAnimationSequence();
  window.setInterval(restartAnimationSequence, 4000);
}

currentUrl.textContent = location.href;
destinationLink.href = destination;
destinationUrl.textContent = destination;

try {
  const parsed = new URL(destination);
  if (parsed.hostname === "wiki.shroudforge.net") {
    const title = parsed.searchParams.get("title") || parsed.pathname.split("/").at(-1);
    destinationTitle.textContent = `Open ${decodeURIComponent(title).replaceAll("_", " ")} on the Wiki`;
  } else {
    destinationTitle.textContent = `Open ${parsed.hostname}`;
  }
} catch {
  destinationTitle.textContent = "Open the new page";
}
