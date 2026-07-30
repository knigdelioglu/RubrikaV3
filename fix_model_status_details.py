import re

f = "src/pages/ModelStatusPage.tsx"
with open(f, "r") as file:
    content = file.read()

# Replace the pre tag for technical details with a details block
pattern = r"\{currentStatus\.lastError\.technicalDetails && \(\s*<pre style=\{\{ marginTop: '0\.5rem', whiteSpace: 'pre-wrap' \}\}>\s*\{currentStatus\.lastError\.technicalDetails\}\s*</pre>\s*\)\}"

replacement = """{currentStatus.lastError.technicalDetails && (
                  <details style={{ marginTop: '0.75rem', fontSize: '0.9em' }}>
                    <summary style={{ cursor: 'pointer', fontWeight: 'bold', color: '#b91c1c' }}>Geliştirici Detayları</summary>
                    <pre style={{ marginTop: '0.5rem', whiteSpace: 'pre-wrap', background: '#fef2f2', padding: '0.5rem', borderRadius: 4 }}>
                      {currentStatus.lastError.technicalDetails}
                    </pre>
                  </details>
                )}"""

content = re.sub(pattern, replacement, content)

with open(f, "w") as file:
    file.write(content)
