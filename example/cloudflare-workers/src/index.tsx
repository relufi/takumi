import { fetchResources } from "@takumi-rs/helpers";
import { ImageResponse } from "@takumi-rs/image-response/wasm";
import { initSync, Renderer } from "@takumi-rs/wasm";
import module from "@takumi-rs/wasm/takumi_wasm_bg.wasm";
import archivo from "../../../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf";
import DocsTemplate from "../../../takumi-template/src/templates/docs-template";

const fetchCache = new Map();
const logoUrl = "https://takumi.kane.tw/logo.svg";

initSync(module);

const renderer = new Renderer({
  fonts: [archivo],
});

export default {
  async fetch(request) {
    const { pathname, searchParams } = new URL(request.url);

    // stop chrome from requesting favicon.ico
    if (pathname === "/favicon.ico") {
      return new Response(null, { status: 204 });
    }

    const name = searchParams.get("name") || "Wizard";

    const fetchedResources = await fetchResources([logoUrl], {
      cache: fetchCache,
    });

    return new ImageResponse(
      <DocsTemplate
        title={`Hello, ${name}`}
        description="This is an example of rendering on Cloudflare Workers!"
        icon={<img tw="w-16" src={logoUrl} alt="Logo" />}
        site="Takumi"
        primaryColor="#F48120"
        primaryTextColor="#fff"
      />,
      {
        fetchedResources,
        width: 1200,
        height: 630,
        format: "png",
        renderer,
      },
    );
  },
} satisfies ExportedHandler<Env>;
