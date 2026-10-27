(() => {
  const configuration = document.getElementById("visualization-share");
  const frame = document.getElementById("codex-visualization");
  if (configuration == null || !(frame instanceof HTMLIFrameElement)) {
    return;
  }
  let config;
  try {
    config = JSON.parse(configuration.textContent);
  } catch {
    return;
  }
  const metadata = config.metadata ?? {};
  const labels = config.labels;
  const element = (tag, className, text) => {
    const node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    if (typeof text === "string") {
      node.textContent = text;
    }
    return node;
  };
  const link = (href, text, className) => {
    const node = element("a", className, text);
    node.href = href;
    node.target = "_blank";
    node.rel = "noopener noreferrer";
    return node;
  };
  const safeUrl = (value) => {
    try {
      const url = new URL(value);
      return url.protocol === "https:" && !url.username && !url.password
        ? url.href
        : null;
    } catch {
      return null;
    }
  };

  // Bundle the reviewed vectors so portable Python/Work exports need no icon
  // package or network request. Geometry comes from these canonical assets:
  // codex/codex-apps/webview/src/icons/chatgpt-wordmark.svg
  // plugins/visualize/assets/visualize.svg and visualize-dark.svg (theme-aware paint)
  // @oai/icons 0.8.65: {sources,arrow-curvedpath-left-md,table,file-code-square,document}-light-20
  // @oai/icons/chevron-right-sm-light-16
  const icons = {
    wordmark:
      '<svg\n  width="357"\n  height="62"\n  viewBox="0 0 357 62"\n  fill="none"\n  role="img"\n  focusable="false"\n  xmlns="http://www.w3.org/2000/svg"\n>\n  <title>ChatGPT</title>\n  <path\n    d="M30.8694 50.7428C19.9894 50.7428 12.4244 42.4978 12.4244 31.1078C12.4244 19.7178 19.9894 11.4728 30.6994 11.4728C38.5194 11.4728 44.5544 16.0628 46.3394 21.6728H58.4944C56.3694 9.43281 44.8944 0.507812 30.5294 0.507812C13.2744 0.507812 0.609375 14.1928 0.609375 31.1078C0.609375 48.0228 12.9344 61.7078 30.6144 61.7078C45.2344 61.7078 56.4544 52.9528 58.7494 40.5428H46.6794C44.6394 46.2378 38.6894 50.7428 30.8694 50.7428Z"\n    fill="currentColor"\n  />\n  <path\n    d="M88.586 17.5094C82.976 17.5094 78.556 19.9744 76.091 23.3744V1.35938H65.041V60.8594H76.091V37.9944C76.091 31.3644 79.661 27.0294 85.441 27.0294C90.711 27.0294 93.686 31.1094 93.686 36.8044V60.8594H104.736V35.0194C104.736 24.4794 98.276 17.5094 88.586 17.5094Z"\n    fill="currentColor"\n  />\n  <path\n    d="M150.04 33.1478C150.04 23.4578 143.41 17.5078 131.17 17.5078C121.14 17.5078 113.15 23.1178 111.79 31.2778H122.925C123.775 28.4728 126.835 26.2628 131.085 26.2628C136.61 26.2628 139.245 29.1528 139.245 34.1678V34.4228L128.45 35.3578C117.145 36.2928 110.855 40.8828 110.855 49.5528C110.855 56.8628 117.145 61.7078 125.9 61.7078C131.765 61.7078 137.46 59.4128 139.84 55.5878C139.84 57.4578 140.01 59.1578 140.35 60.8578H150.635C150.21 58.1378 150.04 54.9928 150.04 51.3378V33.1478ZM139.245 43.2628C139.245 48.2778 135.505 53.3778 127.94 53.3778C123.86 53.3778 121.65 51.4228 121.65 48.5328C121.65 45.3878 123.775 43.4328 128.535 43.0078L139.245 42.0728V43.2628Z"\n    fill="currentColor"\n  />\n  <path\n    d="M173.245 4.58984H162.195V18.3598H153.355V27.2848H162.195V48.1948C162.195 56.2698 166.36 60.8598 175.285 60.8598H184.125V51.6798H178.515C174.775 51.6798 173.245 50.4048 173.245 46.9198V27.2848H184.125V18.3598H173.245V4.58984Z"\n    fill="currentColor"\n  />\n  <path\n    d="M239.033 29.0678H219.993V38.5878H236.228C235.123 46.4928 227.728 51.2528 219.483 51.2528C207.583 51.2528 200.613 42.4978 200.613 31.1078C200.613 19.7178 208.433 11.3878 219.143 11.3878C226.453 11.3878 232.403 15.6378 233.933 20.3978H246.088C243.793 8.58281 232.913 0.507812 218.973 0.507812C201.803 0.507812 189.053 14.3628 189.053 31.1928C189.053 48.0228 200.783 61.7078 218.293 61.7078C226.283 61.7078 233.508 58.1378 236.738 53.2928V60.8578H246.598V36.6328C246.598 31.7878 243.878 29.0678 239.033 29.0678Z"\n    fill="currentColor"\n  />\n  <path\n    d="M281.703 1.35938H255.354V60.8594H266.573V38.5044H281.789C293.519 38.5044 301.849 31.6194 301.849 20.0594C301.849 8.49937 293.518 1.35938 281.703 1.35938ZM280.939 28.6444H266.573V11.5594H280.939C286.719 11.5594 290.458 14.8744 290.458 20.0594C290.458 25.2444 286.719 28.6444 280.939 28.6444Z"\n    fill="currentColor"\n  />\n  <path\n    d="M304.652 1.35938V11.5594H324.967V60.8594H336.187V11.5594H356.587V1.35938H304.652Z"\n    fill="currentColor"\n  />\n</svg>',
    visualize:
      '<svg\n  xmlns="http://www.w3.org/2000/svg"\n  width="24"\n  height="24"\n  fill="none"\n  viewBox="0 0 24 24"\n>\n  <path\n    fill="light-dark(white, #EBEBEB)"\n    d="M6.95984 4H17.0402C17.706 3.99999 18.2656 3.99997 18.7234 4.03738C19.2022 4.0765 19.6571 4.16144 20.089 4.38148C20.7475 4.71703 21.283 5.25247 21.6185 5.91103C21.8386 6.34289 21.9235 6.79785 21.9626 7.27657C22 7.73444 22 8.29396 22 8.95982V15.0402C22 15.706 22 16.2656 21.9626 16.7234C21.9235 17.2021 21.8386 17.6571 21.6185 18.089C21.283 18.7475 20.7475 19.283 20.089 19.6185C19.6571 19.8386 19.2021 19.9235 18.7234 19.9626C18.2656 20 17.706 20 17.0402 20H6.95982C6.29396 20 5.73444 20 5.27657 19.9626C4.79785 19.9235 4.34289 19.8386 3.91103 19.6185C3.25247 19.283 2.71704 18.7475 2.38148 18.089C2.16144 17.6571 2.0765 17.2021 2.03738 16.7234C1.99997 16.2656 1.99999 15.706 2 15.0402V8.95984C1.99999 8.29397 1.99997 7.73445 2.03738 7.27657C2.0765 6.79785 2.16144 6.34289 2.38148 5.91103C2.71704 5.25247 3.25247 4.71704 3.91103 4.38148C4.34289 4.16144 4.79785 4.0765 5.27657 4.03738C5.73445 3.99997 6.29397 3.99999 6.95984 4Z"\n  />\n  <rect x="3" y="5" width="18" height="14" rx="2.5" fill="light-dark(#2E9EFF, #68C4FF)" />\n  <path\n    fill="#F5F5F5"\n    d="M11.2952 7.93832C11.5345 7.28007 12.4655 7.28007 12.7048 7.93832L13.4804 10.0711C13.5562 10.2796 13.7204 10.4438 13.9289 10.5196L16.0617 11.2952C16.7199 11.5345 16.7199 12.4655 16.0617 12.7048L13.9289 13.4804C13.7204 13.5562 13.5562 13.7204 13.4804 13.9289L12.7048 16.0617C12.4655 16.7199 11.5345 16.7199 11.2952 16.0617L10.5196 13.9289C10.4438 13.7204 10.2796 13.5562 10.0711 13.4804L7.93832 12.7048C7.28007 12.4655 7.28007 11.5345 7.93832 11.2952L10.0711 10.5196C10.2796 10.4438 10.4438 10.2796 10.5196 10.0711L11.2952 7.93832Z"\n  />\n</svg>',
    sources:
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="none"><path fill-rule="evenodd" clip-rule="evenodd" d="M14.4365 2.25244C16.2993 2.25257 17.8104 3.76268 17.8105 5.62549C17.8105 7.48845 16.2995 8.99938 14.4365 8.99951C12.8756 8.99936 11.5624 7.9384 11.1777 6.49854H8.75C8.33552 6.49867 8.08566 6.70063 8.00391 6.88428C7.93983 7.02847 7.8974 7.36151 8.37109 7.84424L12.5283 11.1704L12.5537 11.1958C13.3113 11.9535 13.5635 12.8643 13.2119 13.6558C12.8769 14.4089 12.0854 14.8313 11.251 14.8315H8.86914C8.66988 16.5047 7.24717 17.8032 5.52051 17.8032C3.65778 17.803 2.14772 16.2919 2.14746 14.4292C2.14764 12.5664 3.65773 11.0563 5.52051 11.0562C7.06179 11.0562 8.36147 12.0895 8.76465 13.5015H11.251C11.6654 13.5012 11.9155 13.2984 11.9971 13.1147C12.0609 12.9704 12.1026 12.6381 11.6309 12.1567L7.50195 8.85205L7.47266 8.82959L7.44727 8.80322C6.6895 8.04545 6.4373 7.1358 6.78906 6.34424C7.12406 5.59088 7.9154 5.16859 8.75 5.16846H11.0957C11.319 3.52195 12.7288 2.2526 14.4365 2.25244ZM5.52051 12.3862C4.39226 12.3864 3.47771 13.301 3.47754 14.4292C3.4778 15.5574 4.39232 16.473 5.52051 16.4731C6.64885 16.4731 7.56419 15.5575 7.56445 14.4292C7.56428 13.3008 6.6489 12.3862 5.52051 12.3862ZM14.4365 3.58252C13.3083 3.5827 12.3937 4.49724 12.3936 5.62549C12.3936 6.75388 13.3082 7.66926 14.4365 7.66943C15.5649 7.6693 16.4805 6.75391 16.4805 5.62549C16.4803 4.49722 15.5648 3.58265 14.4365 3.58252Z" fill="currentColor"/></svg>',
    back: '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M14.9587 6.00146C15.3258 6.00168 15.6237 6.29937 15.6237 6.6665V9.16064C15.6237 9.81272 15.4102 10.4093 14.9753 10.8442C14.5404 11.2791 13.9437 11.4926 13.2917 11.4927H6.59834L7.96944 12.8628C8.22914 13.1225 8.22914 13.5445 7.96944 13.8042C7.70971 14.0635 7.28761 14.0638 7.02803 13.8042L4.88252 11.6577C4.42772 11.2029 4.42794 10.4643 4.88252 10.0093L7.02803 7.86279C7.28772 7.60311 7.70974 7.60313 7.96944 7.86279C8.22914 8.12249 8.22914 8.5445 7.96944 8.8042L6.61006 10.1626H13.2917C13.6528 10.1625 13.89 10.0487 14.0349 9.90381C14.1797 9.75883 14.2937 9.52183 14.2937 9.16064V6.6665C14.2937 6.29923 14.5914 6.00146 14.9587 6.00146Z" fill="currentColor"/></svg>',
    table:
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="none"><path fill-rule="evenodd" clip-rule="evenodd" d="M15.167 3.08496C16.8227 3.08514 18.1649 4.42733 18.165 6.08301V13.917C18.1649 15.5727 16.8227 16.9149 15.167 16.915H4.83301C3.17733 16.9149 1.83514 15.5727 1.83496 13.917V6.08301C1.83514 4.42733 3.17733 3.08514 4.83301 3.08496H15.167ZM10.665 15.585H15.167C16.0881 15.5848 16.8348 14.8381 16.835 13.917V8.99805H10.665V15.585ZM3.16504 13.917C3.16522 14.8381 3.91187 15.5848 4.83301 15.585H9.33496V8.99805H3.16504V13.917ZM4.83301 4.41504C3.91187 4.41521 3.16521 5.16187 3.16504 6.08301V7.66797H16.835V6.08301C16.8348 5.16187 16.0881 4.41522 15.167 4.41504H4.83301Z" fill="currentColor"/></svg>',
    code: '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M6.20166 7.42334C6.42204 7.12958 6.83951 7.07018 7.1333 7.29053C7.4269 7.51094 7.48641 7.92844 7.26611 8.22217L5.93213 9.99951L7.26611 11.7788C7.486 12.0726 7.42684 12.4892 7.1333 12.7095C6.83956 12.9298 6.42207 12.8703 6.20166 12.5767L4.56885 10.3989C4.39171 10.1626 4.39169 9.83744 4.56885 9.60107L6.20166 7.42334Z" fill="currentColor"/> <path d="M10.4946 7.5249C10.6591 7.19696 11.058 7.06395 11.3862 7.22803C11.7144 7.39235 11.8481 7.79225 11.6841 8.12061L9.50635 12.4751C9.34207 12.8033 8.94212 12.9359 8.61377 12.772C8.28569 12.6076 8.15292 12.2087 8.31689 11.8804L10.4946 7.5249Z" fill="currentColor"/> <path d="M12.8667 7.29053C13.1605 7.07024 13.578 7.12967 13.7983 7.42334L15.4312 9.60107C15.6082 9.83745 15.6083 10.1626 15.4312 10.3989L13.7983 12.5767C13.578 12.8703 13.1605 12.9296 12.8667 12.7095C12.5732 12.4892 12.5139 12.0725 12.7339 11.7788L14.0669 9.99951L12.7339 8.22217C12.5136 7.9284 12.573 7.51092 12.8667 7.29053Z" fill="currentColor"/> <path fill-rule="evenodd" clip-rule="evenodd" d="M14.7495 2.25146C16.4053 2.25146 17.7484 3.59373 17.7485 5.24951V14.7495C17.7485 16.4054 16.4054 17.7485 14.7495 17.7485H5.24951C3.59373 17.7484 2.25146 16.4053 2.25146 14.7495V5.24951C2.25164 3.59384 3.59384 2.25164 5.24951 2.25146H14.7495ZM5.24951 3.58154C4.32837 3.58172 3.58172 4.32838 3.58154 5.24951V14.7495C3.58154 15.6708 4.32827 16.4183 5.24951 16.4185H14.7495C15.6709 16.4185 16.4185 15.6709 16.4185 14.7495V5.24951C16.4183 4.32827 15.6708 3.58154 14.7495 3.58154H5.24951Z" fill="currentColor"/></svg>',
    document:
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="none"><path fill-rule="evenodd" clip-rule="evenodd" d="M11.8809 2.25171C12.7202 2.25171 13.5256 2.585 14.1191 3.17847L15.9883 5.04761C16.5817 5.64115 16.915 6.44654 16.915 7.28589V14.5837C16.9149 16.3316 15.4979 17.7488 13.75 17.7488H6.25C4.50213 17.7488 3.08514 16.3316 3.08496 14.5837V5.41675C3.08496 3.66877 4.50202 2.25171 6.25 2.25171H11.8809ZM6.25 3.58179C5.23656 3.58179 4.41504 4.40331 4.41504 5.41675V14.5837C4.41521 15.597 5.23667 16.4187 6.25 16.4187H13.75C14.7633 16.4187 15.5848 15.597 15.585 14.5837V8.58179H13.334C12.0463 8.58179 11.0021 7.53739 11.002 6.24976V3.58179H6.25ZM12.332 6.24976C12.3322 6.80285 12.7809 7.25171 13.334 7.25171H15.583C15.5742 6.77746 15.3838 6.32408 15.0479 5.98804L13.1787 4.1189C12.9432 3.8834 12.6496 3.71992 12.332 3.6394V6.24976Z" fill="currentColor"/></svg>',
    chevron:
      '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M6.29561 10.2954C6.09064 10.5003 6.09075 10.8325 6.29561 11.0376C6.50064 11.2426 6.83278 11.2426 7.0378 11.0376L9.46847 8.60689C9.80367 8.27169 9.80367 7.72823 9.46847 7.39303L7.0378 4.96236C6.83278 4.75734 6.50064 4.75734 6.29561 4.96236C6.09059 5.16739 6.09059 5.49952 6.29561 5.70455L8.59054 7.99947L6.29561 10.2954Z" fill="currentColor"/></svg>',
  };
  const icon = (svg, className = "visualization-share-icon") => {
    const node = document.importNode(
      new DOMParser().parseFromString(svg, "image/svg+xml").documentElement,
      true,
    );
    node.setAttribute("class", className);
    node.setAttribute("aria-hidden", "true");
    node.setAttribute("focusable", "false");
    return node;
  };

  document.documentElement.classList.add("visualization-share-root");
  document.body.classList.add("visualization-share-page");
  const brand = element("header", "visualization-share-brand");
  const home = link("https://chatgpt.com/");
  home.setAttribute("aria-label", "ChatGPT");
  home.append(icon(icons.wordmark, "visualization-share-wordmark"));
  brand.append(home);
  const main = element("main", "visualization-share-main");
  const article = element("article", "visualization-share-object");
  const card = element("div", "visualization-share-card");
  const chart = element(
    "section",
    "visualization-share-face visualization-share-chart",
  );
  chart.id = "visualization-share-chart";
  chart.setAttribute("aria-label", frame.title);
  chart.setAttribute("aria-hidden", "false");
  const content = element("div", "visualization-share-content");
  content.append(frame);
  chart.append(content);
  card.append(chart);

  const footer = element("footer", "visualization-share-footer");
  const attribution = element("p", "visualization-share-attribution");
  const authorName =
    typeof metadata.authorName === "string" ? metadata.authorName : "";
  const attributionLabel = authorName ? labels.madeBy : labels.madeWith;
  for (const part of attributionLabel.split(/(\{visualize\})/)) {
    if (part === "{visualize}") {
      const growth = link(
        "https://chatgpt.com/?prompt=%40visualize",
        "Visualize",
        "visualization-share-growth",
      );
      growth.prepend(icon(icons.visualize, "visualization-share-logo"));
      attribution.append(growth);
    } else if (part.trim()) {
      attribution.append(
        element(
          "span",
          "",
          part.replaceAll("{author}", () => authorName).trim(),
        ),
      );
    }
  }
  footer.append(attribution);
  const sources = Array.isArray(metadata.sources) ? metadata.sources : [];
  if (sources.length > 0) {
    const panel = element(
      "section",
      "visualization-share-face visualization-share-sources",
    );
    panel.id = "visualization-share-sources";
    panel.inert = true;
    panel.hidden = true;
    panel.setAttribute("aria-hidden", "true");
    const sourceContent = element("div", "visualization-share-content");
    const header = element("div", "visualization-share-sources-header");
    const heading = element("h2", "", labels.sources);
    heading.id = "visualization-share-sources-heading";
    panel.setAttribute("aria-labelledby", heading.id);
    header.append(heading);
    sourceContent.append(header);
    for (const source of sources) {
      if (typeof source.title !== "string") {
        continue;
      }
      const row = element("details", "visualization-share-source");
      const summary = element("summary");
      const documentIcon = element("span", "visualization-share-source-icon");
      const type =
        typeof source.type === "string" ? source.type.toLowerCase() : "";
      let sourceIcon = icons.document;
      if (type === "saved query" || type === "query" || type === "code") {
        sourceIcon = icons.code;
      } else if (
        type === "dataset" ||
        type === "table" ||
        type === "spreadsheet"
      ) {
        sourceIcon = icons.table;
      }
      documentIcon.append(icon(sourceIcon));
      const text = element("span", "visualization-share-source-heading");
      text.append(
        element("span", "visualization-share-source-title", source.title),
      );
      const context = [source.provider, source.type, source.date]
        .filter((value) => typeof value === "string" && value.length > 0)
        .join(" · ");
      if (context) {
        text.append(
          element("span", "visualization-share-source-context", context),
        );
      }
      summary.append(
        documentIcon,
        text,
        icon(icons.chevron, "visualization-share-chevron"),
      );
      row.append(summary);
      if (typeof source.description === "string" && source.description) {
        row.append(
          element(
            "p",
            "visualization-share-source-description",
            source.description,
          ),
        );
      }
      const url = safeUrl(source.url);
      if (url != null) {
        row.append(
          link(url, labels.openSource, "visualization-share-source-link"),
        );
      }
      sourceContent.append(row);
    }
    panel.append(sourceContent);
    card.append(panel);
    const showSources = element(
      "button",
      "visualization-share-toggle",
      labels.sources,
    );
    showSources.type = "button";
    showSources.setAttribute("aria-controls", panel.id);
    showSources.setAttribute("aria-expanded", "false");
    showSources.prepend(icon(icons.sources));
    const showChart = element(
      "button",
      "visualization-share-toggle",
      labels.backToChart,
    );
    showChart.type = "button";
    showChart.setAttribute("aria-controls", chart.id);
    showChart.hidden = true;
    showChart.prepend(icon(icons.back));
    const hideInactiveSources = () => {
      if (!chart.inert && card.getAnimations().length === 0) {
        panel.hidden = true;
      }
    };
    const flip = (show) => {
      // Keep the iframe mounted and the outgoing sources at their full height
      // until the flip finishes. Hidden sources must not size the chart.
      if (show) {
        panel.hidden = false;
      }
      card.classList.toggle("visualization-share-flipped", show);
      showSources.hidden = show;
      showChart.hidden = !show;
      chart.inert = show;
      panel.inert = !show;
      chart.setAttribute("aria-hidden", String(show));
      panel.setAttribute("aria-hidden", String(!show));
      showSources.setAttribute("aria-expanded", String(show));
      (show ? showChart : showSources).focus({ preventScroll: true });
      hideInactiveSources();
    };
    for (const eventType of ["transitionend", "transitioncancel"]) {
      card.addEventListener(eventType, (event) => {
        if (event.target === card) {
          hideInactiveSources();
        }
      });
    }
    showSources.addEventListener("click", () => flip(true));
    showChart.addEventListener("click", () => flip(false));
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && chart.inert) {
        event.preventDefault();
        flip(false);
      }
    });
    const controls = element("div", "visualization-share-controls");
    controls.append(showSources, showChart);
    footer.append(controls);
  }
  article.append(card, footer);
  main.append(article);
  document.body.prepend(brand, main);
})();
