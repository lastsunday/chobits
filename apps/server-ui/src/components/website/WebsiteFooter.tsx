import { ActionIcon, Container, Group } from '@mantine/core';
import logo from '../../assets/logo.svg';
import { PROJECT_SERVER_URL } from '../../config';
import classes from './WebsiteFooter.module.css';
import { useTranslation } from "react-i18next";

export function WebsiteFooter() {

  const { t } = useTranslation();

  return (
    <div className={classes.footer}>
      <Container className={classes.inner}>
        <img className={classes.logo} src={logo}></img>
        <Group gap={0} className={classes.links} justify="flex-end" wrap="nowrap">
          <ActionIcon component='a' size="lg" color="gray" variant="subtle" href="/test/device/test_page.html" title={t('test.device.title')}>
            <div className={`i-file-icons:test-js`}></div>
          </ActionIcon>
          <ActionIcon component='a' size="lg" color="gray" variant="subtle" href="/admin" title={t('admin.title')}>
            <div className={`i-mdi:administrator`}></div>
          </ActionIcon>
          <ActionIcon component='a' size="lg" color="gray" variant="subtle" href={PROJECT_SERVER_URL}>
            <div className={`i-mdi:github`}></div>
          </ActionIcon>
        </Group>
      </Container>
    </div>
  );
}
