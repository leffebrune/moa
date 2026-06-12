declare module "markdown-it" {
  export default class MarkdownIt {
    constructor(options?: {
      breaks?: boolean;
      html?: boolean;
      linkify?: boolean;
    });

    render(source: string): string;
  }
}
