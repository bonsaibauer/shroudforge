import { useMemo } from 'react'
import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'

const markdown = new MarkdownIt({ html: true, linkify: true, breaks: false })
const renderLinkOpen = markdown.renderer.rules.link_open
markdown.renderer.rules.link_open = (tokens, index, options, environment, self) => {
  tokens[index].attrSet('target', '_blank')
  tokens[index].attrSet('rel', 'noopener noreferrer')
  return renderLinkOpen
    ? renderLinkOpen(tokens, index, options, environment, self)
    : self.renderToken(tokens, index, options)
}

export function CatalogMarkdown({ value }: { value: string }) {
  const html = useMemo(() => DOMPurify.sanitize(markdown.render(value), {
    ADD_ATTR: ['target'],
  }), [value])
  return <div className="discover-markdown" dangerouslySetInnerHTML={{ __html: html }} />
}
